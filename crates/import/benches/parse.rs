//! What reading an MS Project file costs (gantt#1337): `import::parse_mspdi` on a finish-to-start
//! chain of 10,000 and of 50,000 tasks, the two sizes of the comparison harness's `chain` shape
//! (`demos/bench/dataset.ts`). The file has the shape `demos/dhtmlx-compare/to-mspdi.ts` writes:
//! a summary, then one task per chain link with a start constraint and a predecessor link (the
//! calendar is left to the default).
//!
//! ```sh
//! cargo bench -p import --bench parse
//! ```

use std::fmt::Write as _;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use import::parse_mspdi;

const SIZES: [u32; 2] = [10_000, 50_000];

fn chain(tasks: u32) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <Project xmlns=\"http://schemas.microsoft.com/project\">\
         <StartDate>2026-03-02T08:00:00</StartDate><Tasks>\
         <Task><UID>1</UID><Name>Project</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary>\
         <Duration>PT0H0M0S</Duration><DurationFormat>7</DurationFormat></Task>",
    );
    for uid in 2..=tasks {
        let (month, day) = (3 + uid / 28 % 9, 1 + uid % 28);
        write!(
            xml,
            "<Task><UID>{uid}</UID><Name>Task {uid}</Name><OutlineLevel>2</OutlineLevel>\
             <Summary>0</Summary><Duration>PT16H0M0S</Duration><DurationFormat>7</DurationFormat>\
             <ConstraintType>4</ConstraintType>\
             <ConstraintDate>2026-{month:02}-{day:02}T08:00:00</ConstraintDate>"
        )
        .expect("a String takes writes");
        if uid > 2 {
            write!(
                xml,
                "<PredecessorLink><PredecessorUID>{}</PredecessorUID><Type>1</Type>\
                 <LinkLag>0</LinkLag><LagFormat>7</LagFormat></PredecessorLink>",
                uid - 1
            )
            .expect("a String takes writes");
        }
        xml.push_str("</Task>");
    }
    xml.push_str("</Tasks></Project>");
    xml
}

fn parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_mspdi");
    group.sample_size(20);
    for size in SIZES {
        let xml = chain(size);
        group.throughput(Throughput::Bytes(xml.len() as u64));
        group.bench_with_input(BenchmarkId::new("chain", size), &xml, |b, xml| {
            b.iter(|| parse_mspdi(xml).expect("chain parses"));
        });
    }
    group.finish();
}

criterion_group!(benches, parse);
criterion_main!(benches);
