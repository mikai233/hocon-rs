use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use hocon_rs::config::Config;
use hocon_rs::parser::HoconParser;
use hocon_rs::parser::read::{StrRead, StreamRead};
use hocon_rs::value::Value;
use std::fs;
use std::io::Cursor;
use std::path::Path;

fn criterion_benchmark(c: &mut Criterion) {
    let path = Path::new("benches/reference.conf");
    let data = fs::read_to_string(path).expect("failed to read benchmark fixture");
    let bytes = data.as_bytes();

    let mut group = c.benchmark_group("parser");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    group.bench_function("pure_parser", |b| {
        b.iter_batched(
            || StrRead::new(data.as_str()),
            |read| {
                let mut parser = HoconParser::new(read);
                parser.parse().unwrap();
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("pure_parser_stream", |b| {
        b.iter_batched(
            || {
                let cursor = Cursor::new(bytes);
                StreamRead::new(cursor)
            },
            |read| {
                let mut parser = HoconParser::new(read);
                parser.parse().unwrap();
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("parse_config", |b| {
        b.iter(|| {
            Config::from_str::<Value>(data.as_str(), None).unwrap();
        });
    });

    group.finish();

    c.bench_function("load_config", |b| {
        b.iter(|| Config::load::<Value>(path, None).unwrap());
    });
}

fn custom_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(std::time::Duration::from_secs(10)) // 测量 10 秒
        .sample_size(100) // 可选：采样数量
}

criterion_group! {
    name = benches;
    config = custom_criterion(); // 使用自定义配置
    targets = criterion_benchmark
}
criterion_main!(benches);
