use poem::{Endpoint, Middleware, Request, Result};

pub struct BasicLog;

impl<E: Endpoint> Middleware<E> for BasicLog {
    type Output = BasicLogImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        BasicLogImpl { ep }
    }
}

pub struct BasicLogImpl<E> {
    ep: E,
}

impl<E: Endpoint> Endpoint for BasicLogImpl<E> {
    type Output = E::Output;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        println!("[{}] {}", req.method(), req.uri());
        self.ep.call(req).await
    }
}
