pub use select_loop_proc::select_loop;

#[doc(hidden)]
pub mod __private {
    pub use futures;
    pub use tokio;

    pub struct AbortOnDrop(futures::future::AbortHandle);

    impl AbortOnDrop {
        pub fn new<F>(task: F) -> Self
        where
            F: futures::Future + Send + 'static,
            F::Output: Send + 'static,
        {
            let (abort_handle, abort_reg) = futures::future::AbortHandle::new_pair();
            let abort_on_drop = AbortOnDrop(abort_handle);
            let fut = futures::future::Abortable::new(task, abort_reg);
            tokio::spawn(fut);
            abort_on_drop
        }
    }

    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort()
        }
    }
}
