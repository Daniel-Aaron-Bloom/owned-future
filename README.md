# owned-future
[![github](https://img.shields.io/badge/Daniel--Aaron--Bloom%2Fowned-future-8da0cb?style=for-the-badge&logo=github&label=github&labelColor=555555)](https://github.com/Daniel-Aaron-Bloom/owned-future)
[![crates.io](https://img.shields.io/crates/v/owned-future.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/owned-future)
[![docs.rs](https://img.shields.io/badge/docs.rs-owned-future-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs)](https://docs.rs/owned-future)
[![build status](https://img.shields.io/github/actions/workflow/status/Daniel-Aaron-Bloom/owned-future/ci.yml?branch=master&style=for-the-badge)](https://github.com/Daniel-Aaron-Bloom/owned-future/actions?query=branch%3Amaster)


This tiny crate contains helpers to turn a borrowed future into an owned one.

## Motivation

Take [`tokio::sync::Notify`] as an example. It's often useful to call [`Notify::notified`] from
the main thread and then pass it to a spawned thread. Doing this guarantees the resulting
[`Notified`] is watching for calls to [`Notify::notify_waiters`] prior to the thread being
spawned. However this isn't possible with as `notified` borrows the `Notify`.

```compile_fail
use std::sync::Arc;
use tokio::sync::Notify;

let notify = Arc::new(Notify::new());

// Spawn a thread that waits to be notified
{
    let notify = notify.clone();
    // Start listening before we spawn
    let notified = notify.notified();
    tokio::spawn(async move {
        // wait for our listen to complete
        notified.await; // <-- fails because we can't move `notified`
    });
}

// notify the waiting threads
notify.notify_waiters();
```

At present, there's no way to do this kind of borrow and then move, and while there are many
crates available to help turn this problem into a self-borrowing one, those solutions require
`unsafe` code with complicated covariance implications. This crate is instead able to solve this
simple case with no `unsafe`, and only 1-2 lines of `unsafe` code for more complex cases with no
covariance problems. Here is the solution to the above problem:


```rust
use std::sync::Arc;
use tokio::sync::Notify;
use owned_future::make;

let notify = Arc::new(Notify::new());

// Spawn a thread that waits to be notified
{
    // Start listening before we spawn
    let get_notified = owned_future::get!(fn(n: &mut Arc<Notify>) -> () {
        n.notified()
    });
    let notified = make(notify.clone(), get_notified);
    tokio::spawn(async move {
        // wait for our listen to complete
        notified.await;
    });
}

// notify the waiting threads
notify.notify_waiters();
```

## Technical Details

So how does this work exactly? Well, while rust usually doesn't let you move a borrowed value,
there's one exception. Pinned futures. Once `async` code has been transformed into a `Pin`ed
future, it can invoke the borrow operation, but still be freely moved around. Essentially what
this crate does is a prettied up version of this:

```skip
let mut wrapped_notified = Box::pin(async move {
    let notified = notify.notified();

    // This prevents us from driving the future to completion on the first poll
    force_pause().await;

    future.await
});

// Drive the future up to just past our `force_pause`
wrapped_notified.poll_once()

tokio::spawn(async move {
    // wait for our listen to complete
    wrapped_notified.await;
});
```

The more complex wrappers have a little bit more machinery to handle auxiliary values and
errors, and the `Async*` helpers need a little bit of pin-projection and poll handling, but
ultimately the core logic boils down to something like the above.

[`tokio::sync::Notify`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html
[`Notify::notified`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notified
[`Notified`]: https://docs.rs/tokio/latest/tokio/sync/futures/struct.Notified.html
[`Notify::notify_waiters`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notify_waiters

## License

Licensed under MIT license ([LICENSE](LICENSE) or https://opensource.org/licenses/MIT)
