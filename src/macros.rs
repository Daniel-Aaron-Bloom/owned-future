/// A macro for quickly implementing [`crate::GetFut`] without capturing.
///
/// If you need capturing, you will have to manually capture what you want in a user-defined struct
/// and implement [`crate::GetFut`] for that struct.
#[macro_export]
macro_rules! get {
    (fn($input:ident: &mut $ity:ty) -> $oty:ty { $($v:tt)+ }) => {{
        $crate::get!(fn<'a>($input: &mut $ity) -> $oty {$($v)+})
    }};
    (fn<$a:lifetime>($input:ident: &mut $ity:ty) -> $oty:ty { $($v:tt)+ }) => {{
        #[derive(Clone, Copy)]
        struct Impl;
        impl $crate::GetFut for Impl {
            type Input = $ity;
            type Output = $oty;
            fn get_fut<$a>(self, $input: &$a mut Self::Input) -> impl $a+Future<Output = Self::Output> {
                $($v)+
            }
        }
        Impl
    }};
}

/// A macro for quickly implementing [`crate::TryGetFut`] without capturing.
///
/// If you need capturing, you will have to manually capture what you want in a user-defined struct
/// and implement [`crate::TryGetFut`] for that struct.
#[macro_export]
macro_rules! try_get {
    (fn($input:ident: &mut $ity:ty) -> Result<($oty:ty, $aty:ty), $ety:ty> { $($v:tt)+ }) => {{
        $crate::try_get!(fn<'a>($input: &mut $ity) -> Result<($oty, $aty), $ety> {$($v)+})
    }};
    (fn<$a:lifetime>($input:ident: &mut $ity:ty) -> Result<($oty:ty, $aty:ty), $ety:ty> { $($v:tt)+ }) => {{
        #[derive(Clone, Copy)]
        struct Impl;
        impl $crate::TryGetFut for Impl {
            type Input = $ity;
            type Output = $oty;
            type Aux = $aty;
            type Error = $ety;
            fn try_get_fut<$a>(self, $input: &$a mut Self::Input) -> Result<(impl $a+Future<Output = Self::Output>, Self::Aux), Self::Error> {
                $($v)+
            }
        }
        Impl
    }};
}

/// A macro for quickly implementing [`crate::AsyncTryGetFut`] without capturing.
///
/// If you need capturing, you will have to manually capture what you want in a user-defined struct
/// and implement [`crate::AsyncTryGetFut`] for that struct.
#[cfg(feature = "async")]
#[macro_export]
macro_rules! async_try_get {
    (async fn($input:ident: &mut $ity:ty) -> Result<($oty:ty, $aty:ty), $ety:ty> { $($v:tt)+ }) => {{
        $crate::async_try_get!(async fn<'a, 'b>($input: &mut $ity) -> Result<($oty, $aty), $ety> {$($v)+})
    }};
    (async fn<$a:lifetime, $b:lifetime>($input:ident: &mut $ity:ty) -> Result<($oty:ty, $aty:ty), $ety:ty> { $($v:tt)+ }) => {{
        #[derive(Clone, Copy)]
        struct Impl;
        impl<$a> $crate::AsyncTryGetFut<$a> for Impl {
            type Input = $ity;
            type Output = $oty;
            type Aux = $aty;
            type Error = $ety;
            async fn async_try_get_fut<$b>(self, $input: &$b mut Self::Input) -> Result<(impl $b+Future<Output = Self::Output>, Self::Aux), Self::Error> {

                $($v)+
            }
        }
        Impl
    }};
}
/// A macro for quickly implementing [`crate::AsyncSendTryGetFut`] without capturing.
///
/// If you need capturing, you will have to manually capture what you want in a user-defined struct
/// and implement [`crate::AsyncSendTryGetFut`] for that struct.
///
/// For now this has to use `Pin<Box<dyn Future>>` due a compiler bug
/// ([rust-lang/rust#100013](https://github.com/rust-lang/rust/issues/100013)).This will
/// change in a future release when this bug is fixed.
#[cfg(feature = "async")]
#[macro_export]
macro_rules! async_send_try_get {
    (fn($input:ident: &mut $ity:ty) -> Result<($oty:ty, $aty:ty), $ety:ty> { $($v:tt)+ }) => {{
        $crate::async_send_try_get!(fn<'a, 'b>($input: &mut $ity) -> Result<($oty, $aty), $ety> {$($v)+})
    }};
    (fn<$a:lifetime, $b:lifetime>($input:ident: &mut $ity:ty) -> Result<($oty:ty, $aty:ty), $ety:ty> { $($v:tt)+ }) => {{

        use alloc::boxed::Box;
        use core::pin::Pin;

        #[derive(Clone, Copy)]
        struct Impl;
        impl<$a> $crate::AsyncSendTryGetFut<$a> for Impl {
            type Input = $ity;
            type Output = $oty;
            type Aux = $aty;
            type Error = $ety;
            fn async_send_try_get_fut<$b>(self, $input: &$b mut Self::Input) -> Pin<Box<dyn $b + Send + Future<Output = Result<(Pin<Box<dyn $b + Send + Future<Output = Self::Output>>>, Self::Aux), Self::Error>>>> {
                Box::pin({$($v)+})
            }
        }
        Impl
    }};
}
