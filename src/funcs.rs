use core::{
    future::pending,
    mem::MaybeUninit,
    pin::Pin,
    sync::atomic::AtomicPtr,
    task::{Context, Poll, Waker},
};

use alloc::boxed::Box;

use crate::{GetFut, TryGetFut};

struct PollOnce {
    completed: bool,
}

impl Future for PollOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.completed {
            self.completed = true;
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// Encapsulate a borrowed future along with it's owner
#[deny(unsafe_code)]
pub fn make<G>(val: G::Input, getter: G) -> Pin<Box<impl Future<Output = G::Output>>>
where
    G: GetFut,
{
    let mut future = Box::pin(async move {
        let mut val = val;
        let future = getter.get_fut(&mut val);
        PollOnce { completed: false }.await;
        future.await
    });

    let _poll = Future::poll(future.as_mut(), &mut Context::from_waker(Waker::noop()));
    debug_assert!(matches!(_poll, Poll::Pending));

    future
}

/// Try to encapsulate a borrowed future along with it's owner
pub fn try_make<G>(
    val: G::Input,
    getter: G,
) -> Result<(Pin<Box<impl Future<Output = G::Output>>>, G::Aux), (G::Input, G::Error)>
where
    G: TryGetFut,
{
    let mut result = MaybeUninit::<Result<G::Aux, (G::Input, G::Error)>>::uninit();
    let result_ptr = AtomicPtr::new(result.as_mut_ptr());
    let mut future = Box::pin(async move {
        let mut val = val;
        let result_ptr = result_ptr.into_inner();

        let err = 'err: {
            match getter.try_get_fut(&mut val) {
                Ok((future, aux)) => {
                    // SAFETY: this is safe to do because `result_ptr` lives in the stack frame
                    // above us and we write to it exactly once prior to the first poll
                    unsafe {
                        result_ptr.write(Ok(aux));
                    }

                    // return `Pending` and pass control back
                    PollOnce { completed: false }.await;

                    return future.await;
                }
                Err(err) => break 'err err,
            }
        };

        // SAFETY: this is safe to do because `result_ptr` lives in the stack frame above us and we
        // write to it exactly once prior to the first poll
        unsafe {
            result_ptr.write(Err((val, err)));
        }

        // return `Pending` and pass control back
        pending::<()>().await;

        // The future should be forgotten and this should never be called
        unreachable!();
    });

    let _poll = Future::poll(future.as_mut(), &mut Context::from_waker(Waker::noop()));
    debug_assert!(matches!(_poll, Poll::Pending));

    // SAFETY: this is safe to do because `result` is always written exactly once and always before
    // the first poll
    let result = unsafe { result.assume_init() };

    result.map(|aux| (future, aux))
}

#[cfg(feature = "async")]
mod async_feature {
    use core::{
        future::pending,
        mem,
        pin::Pin,
        sync::atomic::AtomicPtr,
        task::{Context, Poll},
    };

    use alloc::boxed::Box;
    use pin_project_lite::pin_project;

    use crate::{AsyncSendTryGetFut, AsyncTryGetFut, funcs::PollOnce};

    enum AsyncTryMakeFuture<'a, G: AsyncTryGetFut<'a>> {
        Input(G::Input, G),
        Future(Pin<Box<dyn 'a + Future<Output = G::Output>>>),
        Done,
    }

    pin_project! {
        /// Try to encapsulate an async borrowed future along with it's owner
        pub struct AsyncTry<'a, G: AsyncTryGetFut<'a>> {
            result: Option<Result<G::Aux, (G::Input, G::Error)>>,
            future: AsyncTryMakeFuture<'a, G>
        }
    }

    impl<'a, G: AsyncTryGetFut<'a>> AsyncTry<'a, G> {
        pub fn new(val: G::Input, getter: G) -> Self {
            Self {
                result: None,
                future: AsyncTryMakeFuture::Input(val, getter),
            }
        }
    }

    /// An alias for the output type of [`AsyncTry`]
    ///
    /// This will stop being `Box<dyn _>` once either `type_alias_impl_trait` or
    /// `impl_trait_in_assoc_type` stabilize
    pub type AsyncTryOutput<'a, G> = Result<
        (
            Pin<Box<dyn 'a + Future<Output = <G as AsyncTryGetFut<'a>>::Output>>>,
            <G as AsyncTryGetFut<'a>>::Aux,
        ),
        (
            <G as AsyncTryGetFut<'a>>::Input,
            <G as AsyncTryGetFut<'a>>::Error,
        ),
    >;

    impl<'a, G: AsyncTryGetFut<'a>> Future for AsyncTry<'a, G> {
        type Output = AsyncTryOutput<'a, G>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let future = match mem::replace(&mut self.future, AsyncTryMakeFuture::Done) {
                AsyncTryMakeFuture::Done => unreachable!(),
                AsyncTryMakeFuture::Input(val, getter) => {
                    let result_ptr = AtomicPtr::new(&mut self.result);

                    let mut future = Box::pin(async move {
                        let mut val = val;
                        let result_ptr = result_ptr.into_inner();
                        let err = 'err: {
                            match getter.async_try_get_fut(&mut val).await {
                                Ok((future, aux)) => {
                                    // SAFETY: this is safe to do because `result_ptr` is pinned by our
                                    // poller and we write to it exactly once
                                    unsafe {
                                        debug_assert!((*result_ptr).is_none());
                                        *result_ptr = Some(Ok(aux));
                                    }

                                    // return `Pending` and pass control back
                                    PollOnce { completed: false }.await;

                                    return future.await;
                                }
                                Err(err) => break 'err err,
                            }
                        };
                        // SAFETY: this is safe to do because `result_ptr` is pinned by our poller and
                        // we write to it exactly once
                        unsafe {
                            debug_assert!((*result_ptr).is_none());
                            *result_ptr = Some(Err((val, err)));
                        }

                        // return `Pending` and pass control back
                        pending::<()>().await;

                        // The future should be forgotten and this should never be called
                        unreachable!();
                    });
                    let _result = future.as_mut().poll(cx);
                    debug_assert!(matches!(_result, Poll::Pending));
                    future
                }
                AsyncTryMakeFuture::Future(mut future) => {
                    let _result = future.as_mut().poll(cx);
                    debug_assert!(matches!(_result, Poll::Pending));
                    future
                }
            };

            if let Some(result) = self.result.take() {
                return Poll::Ready(result.map(|aux| (future, aux)));
            }
            self.future = AsyncTryMakeFuture::Future(future);

            Poll::Pending
        }
    }

    enum AsyncSendTryMakeFuture<'a, G: AsyncSendTryGetFut<'a>> {
        Input(G::Input, G),
        Future(Pin<Box<dyn 'a + Send + Future<Output = G::Output>>>),
        Done,
    }

    pin_project! {
        /// Try to encapsulate an async borrowed future along with it's owner
        pub struct AsyncSendTry<'a, G: AsyncSendTryGetFut<'a>> {
            result: Option<Result<G::Aux, (G::Input, G::Error)>>,
            future: AsyncSendTryMakeFuture<'a, G>
        }
    }

    impl<'a, G: AsyncSendTryGetFut<'a>> AsyncSendTry<'a, G> {
        pub fn new(val: G::Input, getter: G) -> Self {
            Self {
                result: None,
                future: AsyncSendTryMakeFuture::Input(val, getter),
            }
        }
    }

    /// An alias for the output type of [`AsyncTry`]
    ///
    /// This will stop being `Box<dyn _>` once either `type_alias_impl_trait` or
    /// `impl_trait_in_assoc_type` stabilize
    pub type AsyncSendTryOutput<'a, G> = Result<
        (
            Pin<Box<dyn 'a + Send + Future<Output = <G as AsyncSendTryGetFut<'a>>::Output>>>,
            <G as AsyncSendTryGetFut<'a>>::Aux,
        ),
        (
            <G as AsyncSendTryGetFut<'a>>::Input,
            <G as AsyncSendTryGetFut<'a>>::Error,
        ),
    >;

    impl<'a, G: AsyncSendTryGetFut<'a>> Future for AsyncSendTry<'a, G> {
        type Output = AsyncSendTryOutput<'a, G>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let future = match mem::replace(&mut self.future, AsyncSendTryMakeFuture::Done) {
                AsyncSendTryMakeFuture::Done => unreachable!(),
                AsyncSendTryMakeFuture::Input(val, getter) => {
                    let result_ptr = AtomicPtr::new(&mut self.result);

                    let mut future = Box::pin(async move {
                        let mut val = val;
                        let err = 'err: {
                            match getter.async_send_try_get_fut(&mut val).await {
                                Ok((future, aux)) => {
                                    let result_ptr = result_ptr.into_inner();
                                    // SAFETY: this is safe to do because `result_ptr` is pinned by our
                                    // poller and we write to it exactly once
                                    unsafe {
                                        debug_assert!((*result_ptr).is_none());
                                        *result_ptr = Some(Ok(aux));
                                    }

                                    // return `Pending` and pass control back
                                    PollOnce { completed: false }.await;

                                    return future.await;
                                }
                                Err(err) => break 'err err,
                            }
                        };
                        let result_ptr = result_ptr.into_inner();
                        // SAFETY: this is safe to do because `result_ptr` is pinned by our poller and
                        // we write to it exactly once
                        unsafe {
                            debug_assert!((*result_ptr).is_none());
                            *result_ptr = Some(Err((val, err)));
                        }

                        // return `Pending` and pass control back
                        pending::<()>().await;

                        // The future should be forgotten and this should never be called
                        unreachable!();
                    });
                    let _result = future.as_mut().poll(cx);
                    debug_assert!(matches!(_result, Poll::Pending));
                    // todo!("blocked on rust-lang/rust#100013")
                    future
                }
                AsyncSendTryMakeFuture::Future(mut future) => {
                    let _result = future.as_mut().poll(cx);
                    debug_assert!(matches!(_result, Poll::Pending));
                    future
                }
            };

            if let Some(result) = self.result.take() {
                return Poll::Ready(result.map(|aux| (future, aux)));
            }
            self.future = AsyncSendTryMakeFuture::Future(future);

            Poll::Pending
        }
    }
}

#[cfg(feature = "async")]
pub use async_feature::*;
