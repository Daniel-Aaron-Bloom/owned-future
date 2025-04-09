use core::pin::Pin;

use alloc::boxed::Box;

/// A functor trait for getting a simple future from an input.
pub trait GetFut {
    /// The input type of the functor.
    type Input;
    /// The output type of the resulting future.
    type Output;

    /// The functor applicator.
    ///
    /// Takes a mutable reference some input and returns a future
    fn get_fut<'a>(self, input: &'a mut Self::Input) -> impl 'a + Future<Output = Self::Output>;
}

/// An extension trait to call [`crate::get`]
pub trait ApplyGetFut: GetFut {
    /// Just calls [`crate::get`]
    fn apply_to(self, val: Self::Input) -> Pin<Box<impl Future<Output = Self::Output>>>;
}

impl<G: GetFut> ApplyGetFut for G {
    fn apply_to(self, val: Self::Input) -> Pin<Box<impl Future<Output = Self::Output>>> {
        crate::make(val, self)
    }
}

/// A functor trait for trying to get a complex future from an input.
pub trait TryGetFut {
    /// The input type of the functor. Will be returned to the caller if unsuccessful.
    type Input;
    /// The output type of the resulting future if successful.
    type Output;
    /// Auxiliary data that should be returned to the caller if successful.
    type Aux;
    /// Error data that should be returned to the caller if unsuccessful.
    type Error;

    /// The functor applicator.
    ///
    /// Takes a mutable reference some input and returns either a future and some auxiliary data
    /// or an error.
    fn try_get_fut<'a>(
        self,
        input: &'a mut Self::Input,
    ) -> Result<(impl 'a + Future<Output = Self::Output>, Self::Aux), Self::Error>;
}

/// An extension trait to call [`crate::try_get`]
pub trait TryApplyGetFut: TryGetFut {
    /// Just calls [`crate::try_get`]
    fn apply_to(
        self,
        val: Self::Input,
    ) -> Result<(Pin<Box<impl Future<Output = Self::Output>>>, Self::Aux), (Self::Input, Self::Error)>;
}

impl<G: TryGetFut> TryApplyGetFut for G {
    fn apply_to(
        self,
        val: Self::Input,
    ) -> Result<(Pin<Box<impl Future<Output = Self::Output>>>, Self::Aux), (Self::Input, Self::Error)>
    {
        crate::try_make(val, self)
    }
}

#[cfg(feature = "async")]
mod async_feature {
    use alloc::boxed::Box;
    use core::pin::Pin;

    use crate::{AsyncSendTryOutput, AsyncTryOutput};

    /// A functor trait for trying to get a complex future from an input.
    pub trait AsyncTryGetFut<'a>: 'a {
        /// The input type of the functor. Will be returned to the caller if unsuccessful.
        type Input: 'a;
        /// The output type of the resulting future if successful.
        type Output: 'a;
        /// Auxiliary data that should be returned to the caller if successful.
        type Aux: 'a;
        /// Error data that should be returned to the caller if unsuccessful.
        type Error: 'a;

        /// The functor applicator.
        ///
        /// Takes a mutable reference some input and returns either a future and some auxiliary data
        /// or an error.
        #[allow(async_fn_in_trait)]
        async fn async_try_get_fut<'b>(
            self,
            input: &'b mut Self::Input,
        ) -> Result<(impl 'b + Future<Output = Self::Output>, Self::Aux), Self::Error>;
    }

    /// An extension trait to call [`crate::AsyncTry`]
    pub trait AsyncTryApplyGetFut<'a>: AsyncTryGetFut<'a> {
        /// Just calls [`crate::try_get`]
        #[allow(async_fn_in_trait)]
        async fn apply_to(self, val: Self::Input) -> AsyncTryOutput<'a, Self>;
    }

    impl<'a, G: AsyncTryGetFut<'a>> AsyncTryApplyGetFut<'a> for G {
        async fn apply_to(self, val: Self::Input) -> AsyncTryOutput<'a, Self> {
            crate::AsyncTry::new(val, self).await
        }
    }

    /// A functor trait for trying to get a complex future from an input.
    pub trait AsyncSendTryGetFut<'a>: 'a + Send {
        /// The input type of the functor. Will be returned to the caller if unsuccessful.
        type Input: 'a + Sync + Send;
        /// The output type of the resulting future if successful.
        type Output: 'a + Send;
        /// Auxiliary data that should be returned to the caller if successful.
        type Aux: 'a + Send;
        /// Error data that should be returned to the caller if unsuccessful.
        type Error: 'a + Send;

        /// The functor applicator.
        ///
        /// Takes a mutable reference some input and returns either a future and some auxiliary data
        /// or an error.
        ///
        /// For now this has to use `Pin<Box<dyn Future>>` due a compiler bug
        /// ([rust-lang/rust#100013](https://github.com/rust-lang/rust/issues/100013)). This will
        /// change in a future release when this bug is fixed.
        fn async_send_try_get_fut<'b>(
            self,
            input: &'b mut Self::Input,
        ) -> Pin<
            Box<
                dyn 'b
                    + Send
                    + Future<
                        Output = Result<
                            (
                                Pin<Box<dyn 'b + Send + Future<Output = Self::Output>>>,
                                Self::Aux,
                            ),
                            Self::Error,
                        >,
                    >,
            >,
        >;
    }

    /// An extension trait to call [`crate::AsyncSendTry`]
    pub trait AsyncSendTryApplyGetFut<'a>: AsyncSendTryGetFut<'a> {
        /// Just calls [`crate::AsyncSendTry`]
        #[allow(async_fn_in_trait)]
        async fn apply_to(self, val: Self::Input) -> AsyncSendTryOutput<'a, Self>;
    }

    impl<'a, G: AsyncSendTryGetFut<'a>> AsyncSendTryApplyGetFut<'a> for G {
        async fn apply_to(self, val: Self::Input) -> AsyncSendTryOutput<'a, Self> {
            crate::AsyncSendTry::new(val, self).await
        }
    }
}

#[cfg(feature = "async")]
pub use async_feature::*;
