//! A lazy stream over every item of a paged list (BEHAVIOR 12).

use std::collections::VecDeque;
use std::fmt;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;

use crate::error::Result;

type PageFuture<T> = Pin<Box<dyn Future<Output = Result<(Vec<T>, Option<String>)>> + Send>>;
type Fetch<T> = Box<dyn FnMut(Option<String>) -> PageFuture<T> + Send>;

/// Every item of a paged list, fetched lazily page by page: a
/// [`Stream`] that follows `next_cursor` until it is `None`.
///
/// A failed page is yielded once as an `Err` and ends the stream. Use it with
/// any `Stream` combinator library, or directly with
/// [`try_next`](Self::try_next):
///
/// ```no_run
/// # async fn run(client: ensemble_weather::Client) -> Result<(), ensemble_weather::Error> {
/// let mut stations = client.catalog_stations().admin1("US-OR").items();
/// while let Some(station) = stations.try_next().await? {
///     println!("{}", station.location.name);
/// }
/// # Ok(()) }
/// ```
#[must_use = "a stream does nothing until polled"]
pub struct Items<T> {
    fetch: Fetch<T>,
    cursor: Option<String>,
    buffer: VecDeque<T>,
    pending: Option<PageFuture<T>>,
    done: bool,
}

impl<T> fmt::Debug for Items<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Items")
            .field("cursor", &self.cursor)
            .field("buffered", &self.buffer.len())
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl<T> Items<T> {
    pub(crate) fn new(
        cursor: Option<String>,
        fetch: impl FnMut(Option<String>) -> PageFuture<T> + Send + 'static,
    ) -> Self {
        Self {
            fetch: Box::new(fetch),
            cursor,
            buffer: VecDeque::new(),
            pending: None,
            done: false,
        }
    }
}

impl<T: Unpin> Items<T> {
    /// The next item, `None` after the last one.
    ///
    /// # Errors
    ///
    /// The error of a page that failed; the stream ends after it.
    pub async fn try_next(&mut self) -> Result<Option<T>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx))
            .await
            .transpose()
    }

    /// Every remaining item, in order.
    ///
    /// # Errors
    ///
    /// The error of the first page that failed.
    pub async fn try_collect(mut self) -> Result<Vec<T>> {
        let mut out = Vec::new();
        while let Some(item) = self.try_next().await? {
            out.push(item);
        }
        Ok(out)
    }
}

impl<T: Unpin> Stream for Items<T> {
    type Item = Result<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(item) = this.buffer.pop_front() {
                return Poll::Ready(Some(Ok(item)));
            }
            if this.done {
                return Poll::Ready(None);
            }
            let fetch = &mut this.fetch;
            let cursor = &this.cursor;
            let pending = this.pending.get_or_insert_with(|| fetch(cursor.clone()));
            let page = match pending.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(page) => page,
            };
            this.pending = None;
            match page {
                Err(e) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
                Ok((items, next)) => {
                    this.buffer.extend(items);
                    match next.filter(|c| !c.is_empty()) {
                        Some(c) => this.cursor = Some(c),
                        None => this.done = true,
                    }
                }
            }
        }
    }
}
