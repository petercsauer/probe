//! Benchmarks for ZMQ/ZMTP decoder performance.

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use prb_core::{DecodeContext, ProtocolDecoder};
use prb_zmq::ZmqDecoder;

fn create_test_zmtp_stream() -> Vec<u8> {
    let mut data = Vec::new();

    // ZMTP 3.0 greeting (64 bytes)
    // Signature: 0xFF + padding(8 zeros) + 0x7F
    data.push(0xFF);
    data.extend_from_slice(&[0x00; 8]);
    data.push(0x7F);

    // Version: major=3, minor=0
    data.push(0x03);
    data.push(0x00);

    // Mechanism: "NULL" (20 bytes, zero-padded)
    let mut mechanism = b"NULL".to_vec();
    mechanism.resize(20, 0);
    data.extend_from_slice(&mechanism);

    // as-server: 0 (client)
    data.push(0x00);

    // Padding to 64 bytes
    data.resize(64, 0);

    // READY command frame
    // flags: 0x04 (COMMAND), length: 1 byte
    let mut ready_body = Vec::new();
    ready_body.push(5); // "READY" length
    ready_body.extend_from_slice(b"READY");

    // Property: Socket-Type = PUB
    ready_body.push(11); // "Socket-Type" length
    ready_body.extend_from_slice(b"Socket-Type");
    ready_body.extend_from_slice(&(3u32).to_be_bytes()); // value length
    ready_body.extend_from_slice(b"PUB");

    data.push(0x04); // flags: COMMAND
    data.push(ready_body.len() as u8);
    data.extend_from_slice(&ready_body);

    // Message frame: topic + payload
    // Frame 1: topic (MORE flag set)
    let topic = b"test.topic";
    data.push(0x01); // flags: MORE
    data.push(topic.len() as u8);
    data.extend_from_slice(topic);

    // Frame 2: payload (no MORE flag)
    let payload = b"test message payload data";
    data.push(0x00); // flags: none
    data.push(payload.len() as u8);
    data.extend_from_slice(payload);

    data
}

fn bench_zmtp_frame_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("zmtp_decode");

    let zmtp_stream = create_test_zmtp_stream();

    group.throughput(Throughput::Bytes(zmtp_stream.len() as u64));
    group.bench_function("decode_frame", |b| {
        b.iter(|| {
            let mut decoder = ZmqDecoder::new();
            let ctx = DecodeContext::new()
                .with_src_addr("192.168.1.1:5555")
                .with_dst_addr("192.168.1.2:5556");
            decoder.decode_stream(black_box(&zmtp_stream), black_box(&ctx))
        });
    });

    group.finish();
}

criterion_group!(benches, bench_zmtp_frame_decode);
criterion_main!(benches);
