use criterion::{criterion_group, criterion_main, Criterion};
use hocon_rs::parser::parser::HoconParser;
use hocon_rs::parser::read::{StrRead, StreamRead};
use std::fs;
use std::io::BufReader;

fn criterion_benchmark(c: &mut Criterion) {
    // 1. 在 benchmark 外部读取文件 (不计时)
    let path = "F:/IdeaProjects/akka/akka-actor/src/main/resources/reference.conf";
    let data = fs::read_to_string(path).expect("failed to read file");
    // 2. 在迭代j中只做函数调用 (计时)
    c.bench_function("pure_parser", |b| {
        b.iter(|| {
            let read = StrRead::new(data.as_str());
            let mut parser = HoconParser::new(read);
            parser.parse().unwrap();
        });
    });
    c.bench_function("pure_parser_stream", |b| {
        b.iter(|| {
            let file = fs::File::open(path).unwrap();
            let read = StreamRead::<_, 1024>::new(BufReader::new(file));
            let mut parser = HoconParser::new(read);
            parser.parse().unwrap();
        });
    });
    // c.bench_function("load_config", |b| {
    //     b.iter(|| {
    //         Config::load::<Value>(path, None).unwrap();
    //     });
    // });
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
