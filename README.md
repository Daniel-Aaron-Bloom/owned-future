# owned-future
[![github](https://img.shields.io/badge/Daniel--Aaron--Bloom%2Fowned-future-8da0cb?style=for-the-badge&logo=github&label=github&labelColor=555555)](https://github.com/Daniel-Aaron-Bloom/owned-future)
[![crates.io](https://img.shields.io/crates/v/owned-future.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/owned-future)
[![docs.rs](https://img.shields.io/badge/docs.rs-owned_future-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs)](https://docs.rs/owned-future)
[![build status](https://img.shields.io/github/actions/workflow/status/Daniel-Aaron-Bloom/owned-future/ci.yml?branch=master&style=for-the-badge)](https://github.com/Daniel-Aaron-Bloom/owned-future/actions?query=branch%3Amaster)


This tiny crate contains helpers to turn a borrowed future into an owned one.

## Motivation

Take [`tokio::sync::Notify`] as an example. A common paradigm is to call [`Notify::notified`]
before a relevant threading update/check, then perform the update/check, and then wait on the
resulting [`Notified`] future. Doing this guarantees the `Notified` is watching for calls to
[`Notify::notify_waiters`] prior to the update/check. This paradigm would be useful when dealing
with thread spawning (i.e. calling `notified` and moving the resulting future into the thread),
but this isn't possible with `notified` as it borrows the `Notify`.

```rust
use std::sync::Arc;
use tokio::sync::Notify;

let notify = Arc::new(Notify::new());

// Spawn a thread that waits to be notified
{
    // Copy the Arc
    let notify = notify.clone();

    // Start listening before we spawn
    let notified = notify.notified();

    // Spawn the thread
    tokio::spawn(async move {
        // Wait for our listen to complete
        notified.await; // <-- fails because we can't move `notified`
    });
}

// Notify the waiting threads
notify.notify_waiters();
```

At present, there's no easy way to do this kind of borrow-then-move. While there are many
crates available to help turn this problem into a self-borrowing one, those solutions require
`unsafe` code with complicated covariance implications. This crate is instead able to solve this
simple case with no `unsafe`, and more complex cases are solved with only 1-2 lines of `unsafe`
code with no covariance meddling. Here is the solution to the above problem:


```rust
use std::sync::Arc;
use tokio::sync::Notify;
use owned_future::make;

// Make the constructor for our future
let get_notified = owned_future::get!(fn(n: &mut Arc<Notify>) -> () {
    n.notified()
});

let notify = Arc::new(Notify::new());

// Spawn a thread that waits to be notified
{
    // Copy the Arc
    let notify = notify.clone();

    // Start listening before we spawn
    let notified = make(notify, get_notified);

    // Spawn the thread
    tokio::spawn(async move {
        // wait for our listen to complete
        notified.await;
    });
}

// notify the waiting threads
notify.notify_waiters();
```

## Technical Details

So how does this work exactly? Rust doesn't usually let you move a borrowed value, but there's
one exception. Pinned `async` blocks. Once a value has been moved into an `async` block, and the
the block has been transformed into a `Pin`ned `Future`, during the execution of the future, the
borrow can be executed, but the pointer anchoring the future can still be freely moved around.
All that may sound a little confusing, but essentially what this crate does is a prettied up
version of this:

```rust
// Copy the Arc
let notify = notify.clone();

let mut wrapped_notified = Box::pin(async move {
    let notified = notify.notified();

    // This prevents us from driving the future to completion on the first poll
    force_pause().await;

    future.await
});

// Drive the future up to just past our `force_pause`.
// This will start listening before we spawn
wrapped_notified.poll_once()

// Spawn the thread
tokio::spawn(async move {
    // wait for our listen to complete
    wrapped_notified.await;
});

// notify the waiting threads
notify.notify_waiters();
```

The more complex wrappers have a little bit more machinery to handle auxiliary values and
errors, and the `Async*` helpers do a little bit of pin-projection and poll handling, but
ultimately the core logic boils down to something like the above.

[`tokio::sync::Notify`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html
[`Notify::notified`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notified
[`Notified`]: https://docs.rs/tokio/latest/tokio/sync/futures/struct.Notified.html
[`Notify::notify_waiters`]: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notify_waiters

## License

Licensed under MIT license ([LICENSE](LICENSE) or https://opensource.org/licenses/MIT)
