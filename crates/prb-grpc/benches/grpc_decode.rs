//! Benchmarks for gRPC decoder performance.

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use prb_core::{DecodeContext, ProtocolDecoder};
use prb_grpc::GrpcDecoder;

fn create_test_grpc_request() -> Vec<u8> {
    // HTTP/2 connection preface + SETTINGS frame + HEADERS frame + DATA frame
    let mut data = Vec::new();

    // HTTP/2 connection preface (PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n)
    data.extend_from_slice(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");

    // SETTINGS frame (type=0x04, flags=0x00, stream_id=0, length=0)
    data.extend_from_slice(&[0x00, 0x00, 0x00]); // length: 0
    data.push(0x04); // type: SETTINGS
    data.push(0x00); // flags: none
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // stream_id: 0

    // HEADERS frame with gRPC request (stream_id=1)
    // Simplified: literal headers without HPACK compression
    let headers = b"\x00\x05:path\x0c/test.Service/Method\x00\x0a:authority\x09localhost\x00\x07:method\x04POST\x00\x07:scheme\x04http";
    data.extend_from_slice(&[0x00, 0x00, headers.len() as u8]); // length
    data.push(0x01); // type: HEADERS
    data.push(0x04); // flags: END_HEADERS
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // stream_id: 1
    data.extend_from_slice(headers);

    // DATA frame with gRPC message (stream_id=1)
    // gRPC Length-Prefixed-Message: [compressed_flag(1)][length(4)][data]
    let message = b"test_request_payload";
    let mut grpc_message = vec![0x00]; // compressed_flag: 0 (not compressed)
    grpc_message.extend_from_slice(&(message.len() as u32).to_be_bytes());
    grpc_message.extend_from_slice(message);

    data.extend_from_slice(&[0x00, 0x00, grpc_message.len() as u8]); // length
    data.push(0x00); // type: DATA
    data.push(0x00); // flags: none
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // stream_id: 1
    data.extend_from_slice(&grpc_message);

    data
}

fn create_test_grpc_response() -> Vec<u8> {
    let mut data = Vec::new();

    // HEADERS frame with gRPC response (stream_id=1)
    let headers = b"\x00\x07:status\x03200\x00\x0ccontent-type\x10application/grpc";
    data.extend_from_slice(&[0x00, 0x00, headers.len() as u8]); // length
    data.push(0x01); // type: HEADERS
    data.push(0x04); // flags: END_HEADERS
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // stream_id: 1
    data.extend_from_slice(headers);

    // DATA frame with gRPC response message (stream_id=1)
    let message = b"test_response_payload";
    let mut grpc_message = vec![0x00]; // compressed_flag: 0
    grpc_message.extend_from_slice(&(message.len() as u32).to_be_bytes());
    grpc_message.extend_from_slice(message);

    data.extend_from_slice(&[0x00, 0x00, grpc_message.len() as u8]); // length
    data.push(0x00); // type: DATA
    data.push(0x00); // flags: none
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // stream_id: 1
    data.extend_from_slice(&grpc_message);

    // HEADERS frame with trailers (stream_id=1, END_STREAM)
    let trailers = b"\x00\x0bgrpc-status\x010";
    data.extend_from_slice(&[0x00, 0x00, trailers.len() as u8]); // length
    data.push(0x01); // type: HEADERS
    data.push(0x05); // flags: END_HEADERS | END_STREAM
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // stream_id: 1
    data.extend_from_slice(trailers);

    data
}

fn bench_grpc_message_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("grpc_decode");

    // Benchmark request decoding
    let grpc_request = create_test_grpc_request();
    group.throughput(Throughput::Bytes(grpc_request.len() as u64));
    group.bench_function("decode_request", |b| {
        b.iter(|| {
            let mut decoder = GrpcDecoder::new();
            let ctx = DecodeContext::new()
                .with_src_addr("192.168.1.1:50051")
                .with_dst_addr("192.168.1.2:50052");
            decoder.decode_stream(black_box(&grpc_request), black_box(&ctx))
        });
    });

    // Benchmark response decoding
    let grpc_response = create_test_grpc_response();
    group.throughput(Throughput::Bytes(grpc_response.len() as u64));
    group.bench_function("decode_response", |b| {
        b.iter(|| {
            let mut decoder = GrpcDecoder::new();
            let ctx = DecodeContext::new()
                .with_src_addr("192.168.1.2:50052")
                .with_dst_addr("192.168.1.1:50051");
            decoder.decode_stream(black_box(&grpc_response), black_box(&ctx))
        });
    });

    group.finish();
}

criterion_group!(benches, bench_grpc_message_decode);
criterion_main!(benches);
