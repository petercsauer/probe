//! Benchmarks for DDS/RTPS decoder performance.

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use prb_core::{DecodeContext, ProtocolDecoder};
use prb_dds::DdsDecoder;

fn create_test_rtps_packet() -> Vec<u8> {
    let mut data = Vec::new();

    // RTPS header (20 bytes)
    data.extend_from_slice(b"RTPS"); // magic
    data.extend_from_slice(&[0x02, 0x03]); // protocol version: 2.3
    data.extend_from_slice(&[0x01, 0x0F]); // vendor ID

    // GUID prefix (12 bytes)
    data.extend_from_slice(&[
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
    ]);

    // INFO_TS submessage (timestamp)
    data.push(0x09); // submessage_id: INFO_TS
    data.push(0x01); // flags: little-endian
    data.extend_from_slice(&8u16.to_le_bytes()); // octets_to_next_header
    data.extend_from_slice(&1234567890u32.to_le_bytes()); // seconds
    data.extend_from_slice(&0x80000000u32.to_le_bytes()); // fraction (0.5 seconds)

    // DATA submessage with user payload
    data.push(0x15); // submessage_id: DATA
    data.push(0x01); // flags: little-endian

    let payload = b"test DDS/RTPS user data payload";
    let data_length = 20 + payload.len(); // header(20) + payload
    data.extend_from_slice(&(data_length as u16).to_le_bytes()); // octets_to_next_header

    // DATA submessage header (20 bytes)
    data.extend_from_slice(&[0x00, 0x00]); // extraFlags
    data.extend_from_slice(&16u16.to_le_bytes()); // octetsToInlineQos

    // reader EntityId (4 bytes)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x04]);

    // writer EntityId (4 bytes) - user entity, not SEDP
    data.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

    // sequence number (8 bytes)
    data.extend_from_slice(&1u64.to_le_bytes());

    // Serialized payload
    data.extend_from_slice(payload);

    data
}

fn bench_rtps_packet_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtps_decode");

    let rtps_packet = create_test_rtps_packet();

    group.throughput(Throughput::Bytes(rtps_packet.len() as u64));
    group.bench_function("decode_packet", |b| {
        b.iter(|| {
            let mut decoder = DdsDecoder::new();
            let ctx = DecodeContext::new()
                .with_src_addr("192.168.1.1:12345")
                .with_dst_addr("239.255.0.1:7400");
            decoder.decode_stream(black_box(&rtps_packet), black_box(&ctx))
        });
    });

    group.finish();
}

criterion_group!(benches, bench_rtps_packet_decode);
criterion_main!(benches);
