mod client_id;
mod client_info;
mod input_file_reader;
mod output_record;
mod output_writer;
mod service;
mod transaction_id;
mod transaction_info;

use crate::{
    input_file_reader::InputFileReader,
    output_record::OutputRecordProvider,
    output_writer::OutputWriter,
    service::{Service, TransactionRecordHandler},
};
use log::{debug, error};
use std::{env, io, process};

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let args_count = args.len() - 1;
    if args_count != 1 {
        error!(
            "Expected 1 argument, {args_count} were provided\n\
                Usage: {} <transactions_file.csv>",
            args[0]
        );
        process::exit(1);
    }

    let mut service = Service::new();
    let transactions_file_path = &args[1];
    debug!("Reading file: {transactions_file_path}");
    let file_reader = InputFileReader::new(transactions_file_path.to_string());
    match file_reader.read_file() {
        Ok(records) => {
            for transaction in records {
                if let Err(err) = service.handle(&transaction).await {
                    error!("transaction failure: {err}");
                }
            }
        }
        Err(err) => {
            error!("{err}");
            process::exit(2);
        }
    }

    let writer = OutputWriter::new();
    if let Err(err) = writer.write(io::stdout(), service.get_records()) {
        error!("failed to write results to output: {err}");
    }
}
