//! A small example on using perbase_lib.
use anyhow::Result;
use perbase_lib::{
    par_granges::{self, RegionProcessor},
    position::{Position, ReadFilter},
};
use rust_htslib::bam::{self, record::Record, Read};
use std::path::PathBuf;

// To use ParGranges you will need to implement a [par_granges::RegionProcessor],
// which requires a single method [par_granges::RegionProcessor::process_region]
// and an associated type P, which is type of the values returned in the Vec by
// `process_region`. The returned `P` objects will be kept in order and serialized
// to the specified output.
struct BasicProcessor<F: ReadFilter> {
    // An indexed bamfile to query for the region we were passed
    bamfile: PathBuf,
    // This is an object that implements [position::ReadFilter] and will be applied to
    // each read
    read_filter: F,
}

// A struct that will hold or filter info and impl ReadFilter
struct BasicReadFilter {
    include_flags: u16,
    exclude_flags: u16,
    min_mapq: u8,
}

// The actual implementation of a read filter
impl ReadFilter for BasicReadFilter {
    // Filter reads based SAM flags and mapping quality, true means pass
    #[inline]
    fn filter_read(&self, read: &Record) -> bool {
        let flags = read.flags();
        (!flags) & &self.include_flags == 0
            && flags & &self.exclude_flags == 0
            && &read.mapq() >= &self.min_mapq
    }
}

impl<F: ReadFilter> RegionProcessor for BasicProcessor<F> {
    type P = Position;

    // This function receives an interval to examine.
    fn process_region(&self, tid: u32, start: u64, stop: u64) -> Vec<Self::P> {
        let mut reader = bam::IndexedReader::from_path(&self.bamfile).expect("Indexed reader");
        let header = reader.header().to_owned();
        // fetch the region
        reader.fetch(tid, start, stop).expect("Fetched ROI");
        // Walk over pileups
        let result: Vec<Position> = reader
            .pileup()
            .flat_map(|p| {
                let pileup = p.expect("Extracted a pileup");
                // Verify that we are within the bounds of the chunk we are iterating on
                // Since pileup will pull reads that overhang edges.
                if (pileup.pos() as u64) >= start && (pileup.pos() as u64) < stop {
                    Some(Position::from_pileup(pileup, &header, &self.read_filter))
                } else {
                    None
                }
            })
            .collect();
        result
    }
}

fn main() -> Result<()> {
    // Create the read filter
    let read_filter = BasicReadFilter {
        include_flags: 0,
        exclude_flags: 3848,
        min_mapq: 20,
    };

    // Create the region processor
    let basic_processor = BasicProcessor {
        bamfile: PathBuf::from("test/test.bam"),
        read_filter: read_filter,
    };

    // Create a par_granges runner
    let par_granges_runner = par_granges::ParGranges::new(
        PathBuf::from("test/test.bam"),       // pass in bam
        None,                                 // optional ref fasta
        Some(PathBuf::from("test/test.bed")), // bedfile to narrow regions
        None,                                 // optional output file, will use stdout outherwise
        None,                                 // optional allowed number of threads, defaults to max
        None,                                 // optional chunksize modification
        basic_processor,
    );

    // Run the processor
    par_granges_runner.process()?;

    Ok(())
}