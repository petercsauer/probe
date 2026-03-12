//! Benchmarks for protocol detection.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use prb_detect::{
    DetectionContext, DetectionEngine, GrpcDetector, GuessCrateDetector, PortMappingDetector,
    ProtocolDetector, RtpsDetector, TransportLayer, ZmtpDetector,
};

fn bench_port_mapping_detector(c: &mut Criterion) {
    let detector = PortMappingDetector::with_defaults();
    let ctx = DetectionContext {
        initial_bytes: &[],
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("port_mapping_tcp_match", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_grpc_detector_preface(c: &mut Criterion) {
    let detector = GrpcDetector;
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let ctx = DetectionContext {
        initial_bytes: preface,
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("grpc_detector_preface_match", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_grpc_detector_heuristic(c: &mut Criterion) {
    let detector = GrpcDetector;
    // HTTP/2 SETTINGS frame
    let frame = [0x00, 0x00, 0x0C, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ctx = DetectionContext {
        initial_bytes: &frame,
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("grpc_detector_heuristic_match", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_grpc_detector_no_match(c: &mut Criterion) {
    let detector = GrpcDetector;
    let ctx = DetectionContext {
        initial_bytes: b"random data",
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("grpc_detector_no_match", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_zmtp_detector(c: &mut Criterion) {
    let detector = ZmtpDetector;
    let greeting = [0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0x01];
    let ctx = DetectionContext {
        initial_bytes: &greeting,
        src_port: 12345,
        dst_port: 5555,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("zmtp_detector_match", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_rtps_detector(c: &mut Criterion) {
    let detector = RtpsDetector;
    let header = b"RTPS\x02\x03";
    let ctx = DetectionContext {
        initial_bytes: header,
        src_port: 12345,
        dst_port: 7400,
        transport: TransportLayer::Udp,
        tls_decrypted: false,
    };

    c.bench_function("rtps_detector_match", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_guess_crate_detector(c: &mut Criterion) {
    let detector = GuessCrateDetector::new();
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let ctx = DetectionContext {
        initial_bytes: preface,
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("guess_crate_detector", |b| {
        b.iter(|| detector.detect(black_box(&ctx)));
    });
}

fn bench_detection_engine_grpc(c: &mut Criterion) {
    let engine = DetectionEngine::with_defaults();
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let ctx = DetectionContext {
        initial_bytes: preface,
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("detection_engine_grpc_match", |b| {
        b.iter(|| engine.detect(black_box(&ctx)));
    });
}

fn bench_detection_engine_zmtp(c: &mut Criterion) {
    let engine = DetectionEngine::with_defaults();
    let greeting = [0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x7F, 0x01];
    let ctx = DetectionContext {
        initial_bytes: &greeting,
        src_port: 12345,
        dst_port: 5555,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("detection_engine_zmtp_match", |b| {
        b.iter(|| engine.detect(black_box(&ctx)));
    });
}

fn bench_detection_engine_rtps(c: &mut Criterion) {
    let engine = DetectionEngine::with_defaults();
    let header = b"RTPS\x02\x03";
    let ctx = DetectionContext {
        initial_bytes: header,
        src_port: 12345,
        dst_port: 7400,
        transport: TransportLayer::Udp,
        tls_decrypted: false,
    };

    c.bench_function("detection_engine_rtps_match", |b| {
        b.iter(|| engine.detect(black_box(&ctx)));
    });
}

fn bench_detection_engine_unknown(c: &mut Criterion) {
    let engine = DetectionEngine::with_defaults();
    let ctx = DetectionContext {
        initial_bytes: b"random data",
        src_port: 12345,
        dst_port: 9999,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    c.bench_function("detection_engine_unknown_fallback", |b| {
        b.iter(|| engine.detect(black_box(&ctx)));
    });
}

criterion_group!(
    benches,
    bench_port_mapping_detector,
    bench_grpc_detector_preface,
    bench_grpc_detector_heuristic,
    bench_grpc_detector_no_match,
    bench_zmtp_detector,
    bench_rtps_detector,
    bench_guess_crate_detector,
    bench_detection_engine_grpc,
    bench_detection_engine_zmtp,
    bench_detection_engine_rtps,
    bench_detection_engine_unknown
);
criterion_main!(benches);
