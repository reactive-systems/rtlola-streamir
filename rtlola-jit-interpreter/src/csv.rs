//! Provides an interface for reading inputs from a csv file and writing verdicts as CSV.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Duration;

use itertools::Itertools;
use streamir_lib::ir::{StreamIr, StreamReference};
use thiserror::Error;

use crate::value::{Value, ValueConvertError};
use crate::verdict::{Change, TotalIncremental};
use crate::Inputs;

pub(crate) type ParseValueFn = Box<dyn Fn(&[u8]) -> Result<Option<Value>, ValueConvertError>>;

/// Contains a csv file and can produce new values according to the rows in the file.
pub struct CsvEventSource<R: Read> {
    reader: csv::ByteRecordsIntoIter<R>,
    parser: Vec<ParseValueFn>,
}

impl<R: Read> std::fmt::Debug for CsvEventSource<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsvEventSource").finish()
    }
}

#[derive(Error, Debug)]
/// An error that can happen while reading or writing from or to a csv file
pub enum CsvError {
    #[error("Error while parsing csv file")]
    /// An error happened while reading/writing CSV
    CsvError(#[from] csv::Error),
    #[error("Error while converting to value")]
    /// An error occured while converting the CSV value into a [Value]
    ValueError(#[from] ValueConvertError),
    #[error("The csv file was missing a column")]
    /// The csv file is missing a column
    MissingColumn,
    #[error("The csv output includes parameterized stream \"{0}\"")]
    /// The csv writer tried to write a parameterized output stream
    Parameterized(String),
}

impl<R: Read> CsvEventSource<R> {
    /// Construct a new [CsvEventSource] reading CSV rows from `read`.
    pub fn new(read: R, ir: &StreamIr) -> Self {
        let parser = ir
            .inputs()
            .sorted()
            .map(|i| {
                let ty = ir.stream_memory(StreamReference::In(i)).ty.clone();
                Value::bytes_parser(ty)
            })
            .collect();

        Self {
            reader: csv::Reader::from_reader(read).into_byte_records(),
            parser,
        }
    }

    /// Return the next event (with the associated timestamp) from the CSV file.
    pub fn next_event(&mut self) -> Result<Option<(Inputs, Duration)>, CsvError> {
        let Some(record) = self.reader.next() else {
            return Ok(None);
        };

        let record = record?;
        let mut record = record.into_iter();
        let inputs = self
            .parser
            .iter()
            .map(|p| Ok(p(record.next().ok_or(CsvError::MissingColumn)?)?))
            .collect::<Result<Vec<_>, CsvError>>()?;

        let ts = record.next().ok_or(CsvError::MissingColumn)?;
        let ts: f64 = String::from_utf8_lossy(ts).trim().parse().unwrap();
        let ts = Duration::from_secs_f64(ts);

        Ok(Some((Inputs(inputs), ts)))
    }
}

/// Provides a way to write verdicts as CSV rows
#[derive(Debug)]
pub struct CsvVerdictSink<W: Write> {
    writer: csv::Writer<W>,
    fields: HashMap<StreamReference, usize>,
    has_input: bool,
    has_output: bool,
}

impl<W: Write> CsvVerdictSink<W> {
    /// Construct a new [CsvVerdictSink] writing to `write`.
    pub fn new(write: W, ir: &StreamIr, fields: &[StreamReference]) -> Result<Self, CsvError> {
        let field_map = fields.iter().enumerate().map(|(i, sr)| (*sr, i)).collect();

        let mut writer = csv::Writer::from_writer(write);
        let header = fields
            .iter()
            .map(|sr| ir.name(*sr))
            .chain(std::iter::once("time"));

        writer.write_record(header)?;

        let has_input = fields
            .iter()
            .any(|field| matches!(field, StreamReference::In(_)));
        let has_output = fields
            .iter()
            .any(|field| matches!(field, StreamReference::Out(_)));

        for field in fields {
            if let StreamReference::Out(_) = field {
                if ir.stream_memory(*field).parameters().is_some() {
                    return Err(CsvError::Parameterized(ir.name(*field).to_owned()));
                }
            }
        }

        Ok(Self {
            writer,
            fields: field_map,
            has_input,
            has_output,
        })
    }

    /// Accepts a verdict returned by the monitor and writes it to CSV.
    pub fn accept_verdict(
        &mut self,
        ts: Duration,
        verdict: TotalIncremental,
    ) -> Result<(), CsvError> {
        if !(self.has_input || self.has_output) {
            return Ok(());
        }

        let mut fields = vec![None; self.fields.len()];
        if self.has_input {
            for (sr, value) in &verdict.inputs {
                if let Some(idx) = self.fields.get(&StreamReference::In(*sr)) {
                    fields[*idx] = Some(value)
                }
            }
        }
        if self.has_output {
            for (sr, changes) in &verdict.outputs {
                if let Some(idx) = self.fields.get(&sr.sr()) {
                    if let Some(value) = changes.iter().find_map(|change| match change {
                        Change::Value(inst, value) => {
                            debug_assert!(inst.is_none());
                            Some(value)
                        }
                        _ => None,
                    }) {
                        fields[*idx] = Some(value)
                    }
                }
            }
        }

        if fields.iter().all(|field| field.is_none()) {
            return Ok(());
        }

        let fields = fields
            .into_iter()
            .map(|value| match value {
                Some(value) => value.to_string(),
                None => "#".into(),
            })
            .chain(std::iter::once(ts.as_secs_f64().to_string()));

        self.writer.write_record(fields)?;
        self.writer.flush().unwrap();
        Ok(())
    }
}
