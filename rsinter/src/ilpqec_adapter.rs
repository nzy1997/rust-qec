use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;

use rilpqec::{BackendKind, IlpDecoderConfig};
use rstim::dem::DetectorErrorModel;

use crate::decode::{CompiledDecoder, Decoder};

pub struct IlpDemDecoder {
    config: IlpDecoderConfig,
}

impl IlpDemDecoder {
    pub fn new(config: IlpDecoderConfig) -> Self {
        Self { config }
    }
}

impl Default for IlpDemDecoder {
    fn default() -> Self {
        let mut config = IlpDecoderConfig::default();
        config.backend.kind = BackendKind::Auto;
        Self::new(config)
    }
}

struct CompiledIlpDemDecoder {
    requests: Option<SyncSender<DecodeRequest>>,
    worker: Option<JoinHandle<()>>,
}

struct DecodeRequest {
    dets: Vec<u8>,
    num_shots: usize,
    num_dets: usize,
    num_obs: usize,
    response: SyncSender<Result<Vec<u8>, String>>,
}

impl Decoder for IlpDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        let dem_text = dem.to_string();
        let config = self.config.clone();
        let (requests, request_receiver) = sync_channel::<DecodeRequest>(0);
        let (ready_sender, ready_receiver) = sync_channel(0);
        let worker = std::thread::Builder::new()
            .name("rsinter-rilpqec".into())
            .spawn(move || {
                let decoder = DetectorErrorModel::parse(&dem_text)
                    .map_err(|error| error.to_string())
                    .and_then(|dem| {
                        rilpqec::IlpDemDecoder::from_dem(&dem, config)
                            .map_err(|error| error.to_string())
                    })
                    .and_then(|decoder| decoder.into_compiled().map_err(|error| error.to_string()));
                let mut decoder = match decoder {
                    Ok(decoder) => {
                        if ready_sender.send(Ok(())).is_err() {
                            return;
                        }
                        decoder
                    }
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                };
                while let Ok(request) = request_receiver.recv() {
                    let result = decoder
                        .decode_batch_bit_packed(
                            &request.dets,
                            request.num_shots,
                            request.num_dets,
                            request.num_obs,
                        )
                        .map_err(|error| error.to_string());
                    let _ = request.response.send(result);
                }
            })
            .map_err(|error| format!("failed to start rilpqec worker: {error}"))?;
        match ready_receiver.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = worker.join();
                return Err(error);
            }
            Err(error) => {
                let _ = worker.join();
                return Err(format!(
                    "rilpqec worker stopped during compilation: {error}"
                ));
            }
        }
        Ok(Box::new(CompiledIlpDemDecoder {
            requests: Some(requests),
            worker: Some(worker),
        }))
    }
}

impl CompiledDecoder for CompiledIlpDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
        let (response, response_receiver) = sync_channel(0);
        self.requests
            .as_ref()
            .ok_or_else(|| "rilpqec worker is unavailable".to_string())?
            .send(DecodeRequest {
                dets: dets.to_vec(),
                num_shots,
                num_dets,
                num_obs,
                response,
            })
            .map_err(|error| format!("failed to submit rilpqec batch: {error}"))?;
        response_receiver
            .recv()
            .map_err(|error| format!("rilpqec worker stopped while decoding: {error}"))?
    }
}

impl Drop for CompiledIlpDemDecoder {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
