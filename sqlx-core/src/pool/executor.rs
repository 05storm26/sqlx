use crate::{
    backend::Backend, describe::Describe, executor::Executor, params::IntoQueryParameters,
    pool::Pool, row::FromRow,
};
use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_util::StreamExt;

impl<DB> Executor for Pool<DB>
where
    DB: Backend,
{
    type Backend = DB;

    fn execute<'e, 'q: 'e>(
        &'e mut self,
        query: &'q str,
        params: DB::QueryParameters,
    ) -> BoxFuture<'e, crate::Result<u64>> {
        Box::pin(async move { <&Pool<DB> as Executor>::execute(&mut &*self, query, params).await })
    }

    fn fetch<'e, 'q: 'e, T: 'e>(
        &'e mut self,
        query: &'q str,
        params: DB::QueryParameters,
    ) -> BoxStream<'e, crate::Result<T>>
    where
        T: FromRow<Self::Backend> + Send + Unpin,
    {
        Box::pin(async_stream::try_stream! {
            let mut self_ = &*self;
            let mut s = <&Pool<DB> as Executor>::fetch(&mut self_, query, params);

            while let Some(row) = s.next().await.transpose()? {
                yield row;
            }
        })
    }

    fn fetch_optional<'e, 'q: 'e, T: 'e>(
        &'e mut self,
        query: &'q str,
        params: DB::QueryParameters,
    ) -> BoxFuture<'e, crate::Result<Option<T>>>
    where
        T: FromRow<Self::Backend> + Send,
    {
        Box::pin(async move {
            <&Pool<DB> as Executor>::fetch_optional(&mut &*self, query, params).await
        })
    }

    fn describe<'e, 'q: 'e>(
        &'e mut self,
        query: &'q str,
    ) -> BoxFuture<'e, crate::Result<Describe<Self::Backend>>> {
        Box::pin(async move { <&Pool<DB> as Executor>::describe(&mut &*self, query).await })
    }
}

impl<DB> Executor for &'_ Pool<DB>
where
    DB: Backend,
{
    type Backend = DB;

    fn execute<'e, 'q: 'e>(
        &'e mut self,
        query: &'q str,
        params: DB::QueryParameters,
    ) -> BoxFuture<'e, crate::Result<u64>> {
        Box::pin(async move { self.acquire().await?.execute(query, params).await })
    }

    fn fetch<'e, 'q: 'e, T: 'e>(
        &'e mut self,
        query: &'q str,
        params: DB::QueryParameters,
    ) -> BoxStream<'e, crate::Result<T>>
    where
        T: FromRow<Self::Backend> + Send + Unpin,
    {
        Box::pin(async_stream::try_stream! {
            let mut live = self.acquire().await?;
            let mut s = live.fetch(query, params);

            while let Some(row) = s.next().await.transpose()? {
                yield row;
            }
        })
    }

    fn fetch_optional<'e, 'q: 'e, T: 'e>(
        &'e mut self,
        query: &'q str,
        params: DB::QueryParameters,
    ) -> BoxFuture<'e, crate::Result<Option<T>>>
    where
        T: FromRow<Self::Backend> + Send,
    {
        Box::pin(async move { self.acquire().await?.fetch_optional(query, params).await })
    }

    fn describe<'e, 'q: 'e>(
        &'e mut self,
        query: &'q str,
    ) -> BoxFuture<'e, crate::Result<Describe<Self::Backend>>> {
        Box::pin(async move { self.acquire().await?.describe(query).await })
    }
}
