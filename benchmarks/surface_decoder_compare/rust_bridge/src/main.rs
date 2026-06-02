use std::io::{self, Read};

use surface_decoder_compare_bridge::protocol::{BridgeRequest, BridgeResponse};
use surface_decoder_compare_bridge::run::handle_request;

fn main() {
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin).unwrap();
    let response = match serde_json::from_str::<BridgeRequest>(&stdin) {
        Ok(request) => handle_request(request),
        Err(error) => BridgeResponse::error(format!("invalid request: {error}")),
    };
    println!("{}", serde_json::to_string(&response).unwrap());
}
