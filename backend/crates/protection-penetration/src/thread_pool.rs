use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::{PenetrationResult, PenetrationSimulator, ProtectiveMaterial};

pub struct SimulationRequest {
    pub material: ProtectiveMaterial,
    pub temp: f64,
    pub humidity: f64,
    pub porosity: f64,
    pub concentration: f64,
    pub hours: f64,
}

enum WorkerMessage {
    Task(SimulationRequest, Sender<PenetrationResult>),
    Shutdown,
}

pub struct PenetrationThreadPool {
    sender: Sender<WorkerMessage>,
    workers: Vec<JoinHandle<()>>,
}

impl PenetrationThreadPool {
    pub fn new(num_workers: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<WorkerMessage>();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let rx = Arc::clone(&receiver);
            let handle = thread::spawn(move || {
                let simulator = PenetrationSimulator::new();
                loop {
                    let msg = {
                        let lock = rx.lock().unwrap();
                        lock.recv()
                    };
                    match msg {
                        Ok(WorkerMessage::Task(req, result_tx)) => {
                            let result = simulator.simulate(
                                &req.material,
                                req.temp,
                                req.humidity,
                                req.porosity,
                                req.concentration,
                                req.hours,
                                None,
                            );
                            let _ = result_tx.send(result);
                        }
                        Ok(WorkerMessage::Shutdown) | Err(_) => break,
                    }
                }
            });
            workers.push(handle);
        }

        PenetrationThreadPool { sender, workers }
    }

    pub fn submit(&self, request: SimulationRequest) -> Receiver<PenetrationResult> {
        let (result_tx, result_rx) = mpsc::channel::<PenetrationResult>();
        self.sender.send(WorkerMessage::Task(request, result_tx)).unwrap();
        result_rx
    }
}

impl Drop for PenetrationThreadPool {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.sender.send(WorkerMessage::Shutdown);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
