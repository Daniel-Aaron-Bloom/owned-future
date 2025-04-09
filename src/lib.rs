//!
//! This tiny crate contains helpers to turn a borrowed future into an owned one.
//!
//! # Motivation
//!
//! Take [`tokio::sync::Notify`] as an example. It's often useful to call [`Notify::notified`] from
//! the main thread and then pass it to a spawned thread. Doing this guarantees the resulting
//! [`Notified`] is watching for calls to [`Notify::notify_waiters`] prior to the thread being
//! spawned. However this isn't possible with as `notified` borrows the `Notify`.
//!
//! ```compile_fail
//! use std::sync::Arc;
//! use tokio::sync::Notify;
//!
//! let notify = Arc::new(Notify::new());
//!
//! // Spawn a thread that waits to be notified
//! {
//!     let notify = notify.clone();
//!     // Start listening before we spawn
//!     let notified = notify.notified();
//!     tokio::spawn(async move {
//!         // wait for our listen to complete
//!         notified.await; // <-- fails because we can't move `notified`
//!     });
//! }
//!
//! // notify the waiting threads
//! notify.notify_waiters();
//! ```
//!
//! At present, there's no way to do this kind of borrow and then move, and while there are many
//! crates available to help turn this problem into a self-borrowing one, those solutions require
//! `unsafe` code with complicated covariance implications. This crate is instead able to solve this
//! simple case with no `unsafe`, and only 1-2 lines of `unsafe` code for more complex cases with no
//! covariance problems. Here is the solution to the above problem:
//!
//!
//! ```
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! use std::sync::Arc;
//! use tokio::sync::Notify;
//! use owned_future::make;
//!
//! let notify = Arc::new(Notify::new());
//!
//! // Spawn a thread that waits to be notified
//! {
//!     // Start listening before we spawn
//!     let get_notified = owned_future::get!(fn(n: &mut Arc<Notify>) -> () {
//!         n.notified()
//!     });
//!     let notified = make(notify.clone(), get_notified);
//!     tokio::spawn(async move {
//!         // wait for our listen to complete
//!         notified.await;
//!     });
//! }
//!
//! // notify the waiting threads
//! notify.notify_waiters();
//! # }
//! ```
//!
//! # Technical Details
//!
//! So how does this work exactly? Well, while rust usually doesn't let you move a borrowed value,
//! there's one exception. Pinned futures. Once `async` code has been transformed into a `Pin`ed
//! future, it can invoke the borrow operation, but still be freely moved around. Essentially what
//! this crate does is a prettied up version of this:
//!
//! ```skip
//! let mut wrapped_notified = Box::pin(async move {
//!     let notified = notify.notified();
//!
//!     // This prevents us from driving the future to completion on the first poll
//!     force_pause().await;
//!
//!     future.await
//! });
//!
//! // Drive the future up to just past our `force_pause`
//! wrapped_notified.poll_once()
//!
//! tokio::spawn(async move {
//!     // wait for our listen to complete
//!     wrapped_notified.await;
//! });
//! ```
//!
//! The more complex wrappers have a little bit more machinery to handle auxiliary values and
//! errors, and the `Async*` helpers need a little bit of pin-projection and poll handling, but
//! ultimately the core logic boils down to something like the above.
//!
//! [`tokio::sync::Notify`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html
//! [`Notify::notified`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notified
//! [`Notified`]: https://docs.rs/tokio/latest/tokio/sync/futures/struct.Notified.html
//! [`Notify::notify_waiters`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notify_waiters

#![no_std]

extern crate alloc;

mod funcs;
mod macros;
mod traits;

pub use funcs::*;
pub use traits::*;

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use tokio::sync::{Barrier, Notify};

    use super::*;

    #[tokio::test]
    async fn get() {
        let notify = Arc::new(Notify::new());
        let barrier = Arc::new(Barrier::new(11));

        let get_notified = get!(fn(n: &mut Arc<Notify>) -> () {
            n.notified()
        });

        for _ in 0..10 {
            let barrier = barrier.clone();
            let notified = get_notified.apply_to(notify.clone());
            tokio::spawn(async move {
                notified.await;
                barrier.wait().await;
            });
        }

        notify.notify_waiters();
        barrier.wait().await;
    }

    #[tokio::test]
    async fn try_get() {
        let notify = Arc::new(Notify::new());
        let barrier = Arc::new(Barrier::new(11));

        let get_notified = try_get!(fn(n: &mut Arc<Notify>) -> Result<((), ()), ()> {
            Ok((n.notified(), ()))
        });

        for _ in 0..10 {
            let barrier = barrier.clone();
            let notified = match get_notified.apply_to(notify.clone()) {
                Ok((notified, ())) => notified,
                Err((_notify, ())) => unreachable!(),
            };
            tokio::spawn(async move {
                notified.await;
                barrier.wait().await;
            });
        }

        notify.notify_waiters();
        barrier.wait().await;
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_try_get() {
        use alloc::vec;

        let notify = Arc::new(Notify::new());

        let get_notified = async_try_get!(async fn(n: &mut Arc<Notify>) -> Result<((), ()), ()> {
            Ok((n.notified(), ()))
        });

        let mut waiters = vec![];
        for _ in 0..10 {
            let notified = match get_notified.apply_to(notify.clone()).await {
                Ok((notified, ())) => notified,
                Err((_notify, ())) => unreachable!(),
            };
            waiters.push(async move {
                notified.await;
            })
        }

        notify.notify_waiters();
        for waiter in waiters {
            waiter.await
        }
    }

    #[tokio::test]
    async fn async_send_try_get() {
        let notify = Arc::new(Notify::new());
        let barrier = Arc::new(Barrier::new(11));

        let get_notified = async_send_try_get!(fn<'a, 'b>(n: &mut Arc<Notify>) -> Result<((), ()), ()> {
            async move {
                let n = n.notified();
                let v: Pin<Box<dyn 'b + Send + Future<Output = ()>>> = Box::pin(async move { n.await });
                Ok((
                    v,
                    (),
                ))
            }
        });

        for _ in 0..10 {
            let barrier = barrier.clone();
            let notified = match get_notified.apply_to(notify.clone()).await {
                Ok((notified, ())) => notified,
                Err((_notify, ())) => unreachable!(),
            };
            tokio::spawn(async move {
                notified.await;
                barrier.wait().await;
            });
        }

        notify.notify_waiters();
        barrier.wait().await;
    }
}
