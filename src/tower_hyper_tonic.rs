use std::{
    future::Future,
    task::{Context, Poll},
};

//
// https://www.fpcomplete.com/blog/axum-hyper-tonic-tower-part1/
//

/*
    A Tower Service represents a function from Input to Future<Output>
    You can ask it whether its ready, and you can use `call` to obtain a future from a input.
    For a server, `call` contains the input-handling logic.
*/
pub trait TowerService<Input> {
    type Output;
    type Error;

    fn call(&mut self, req: Input) -> dyn Future<Output = Result<Self::Output, Self::Error>>;

    fn poll_ready(&mut self, ctx: Context) -> Poll<Result<(), Self::Error>>;
}

//
// https://www.fpcomplete.com/blog/axum-hyper-tonic-tower-part2/
//

/*

    Tower itself refers to input/output as request/response. But that's misleading: a tower
    service is more abstract than that, representing an async function from input to output, which may fail.

    hyper creates Http services as Tower services.

    We create:
    - An App that is a Service from Request to Future<Response>
    - An AppFactory that is a Service from ConnectionInfo -> Future<App>

    And we can create app_factory_fn and app_fn that create the instances from closures.
*/

//
// https://www.fpcomplete.com/blog/axum-hyper-tonic-tower-part3/
//

use tonic::transport::Channel;

fn tonic_client_example() {
    let cert = std::fs::read_to_string("ca.pem")?;

    let mut channel = Channel::from_static("https://example.com")
        .tls_config(
            ClientTlsConfig::new()
                .ca_certificate(Certificate::from_pem(&cert))
                .domain_name("example.com".to_string()),
        )?
        .timeout(Duration::from_secs(5))
        .rate_limit(5, Duration::from_secs(1))
        .concurrency_limit(256)
        .connect()
        .await?;

    channel.call(Request::new(BoxBody::empty())).await?;
}
