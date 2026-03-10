# Troubleshooting

Common issues, solutions, and performance tuning tips.

## Build Issues

### `pcap.h not found` or `cannot find -lpcap`

Install libpcap development headers:

```bash
# macOS
xcode-select --install

# Debian / Ubuntu
sudo apt install libpcap-dev

# Fedora / RHEL
sudo dnf install libpcap-devel

# Arch
sudo pacman -S libpcap
```

### `protoc not found`

The `protoc` compiler is only needed if you load `.proto` schema files (not `.desc`). Install it:

```bash
# macOS
brew install protobuf

# Debian / Ubuntu
sudo apt install protobuf-compiler

# Or download from GitHub releases
```

### Compilation errors on Rust < 1.85

PRB requires Rust 2024 edition (1.85+). Update your toolchain:

```bash
rustup update stable
```

## Runtime Issues

### No events decoded from PCAP

1. **Verify the PCAP contains supported traffic:**

```bash
# Check with tcpdump
tcpdump -r capture.pcap -c 10

# Check for gRPC (port 50051)
tcpdump -r capture.pcap port 50051

# Check for any TCP traffic
tcpdump -r capture.pcap tcp
```

2. **Try forcing protocol detection:**

```bash
prb ingest capture.pcap --protocol grpc
prb ingest capture.pcap --protocol zmtp
prb ingest capture.pcap --protocol rtps
```

3. **Enable debug logging:**

```bash
RUST_LOG=debug prb ingest capture.pcap
```

4. **Check if traffic is TLS-encrypted** -- encrypted traffic appears as undecoded TCP streams without the keylog file:

```bash
prb ingest capture.pcap --tls-keylog keys.log
```

### Permission denied on live capture

Live capture requires root (or `CAP_NET_RAW` on Linux):

```bash
# Run as root
sudo prb capture -i eth0

# Or set capabilities (Linux)
sudo setcap cap_net_raw+ep target/release/prb
prb capture -i eth0
```

### No interfaces found

```bash
# List interfaces
prb capture --list-interfaces

# If empty, check permissions
sudo prb capture --list-interfaces
```

### TUI doesn't render correctly

- Ensure your terminal supports 256 colors (most modern terminals do)
- Try a different terminal emulator (iTerm2, Alacritty, WezTerm)
- Set `TERM=xterm-256color` if colors are wrong
- Check terminal size: the TUI needs at least ~80x24

### "Unknown transport kind" in filter

Transport names are case-sensitive in query expressions:

```bash
# Correct
--where 'transport == "gRPC"'
--where 'transport == "ZMQ"'
--where 'transport == "DDS-RTPS"'

# Incorrect
--where 'transport == "grpc"'   # lowercase won't match
```

## Performance Tuning

### Large PCAP files are slow

1. **Increase parallelism:**

```bash
prb ingest large.pcap --jobs 8
```

2. **Use sequential mode for debugging bottlenecks:**

```bash
RUST_LOG=prb_pcap=debug prb ingest large.pcap --jobs 1
```

3. **Pre-filter with BPF at capture time** to reduce file size:

```bash
tcpdump -i eth0 -w filtered.pcap port 50051
prb ingest filtered.pcap
```

### High memory usage

The parallel pipeline buffers packets per flow. For captures with many concurrent connections:

- Reduce `--jobs` to lower the number of concurrent shard buffers
- Split large captures into smaller files
- Use BPF filters to limit captured traffic

### Live capture drops packets

Increase the kernel buffer size:

```bash
sudo prb capture -i eth0 --buffer-size 67108864  # 64 MB
```

On Linux, also increase the system-wide buffer limit:

```bash
sudo sysctl -w net.core.rmem_max=67108864
```

## Platform Notes

### macOS

- libpcap is included with Xcode Command Line Tools
- Live capture requires `sudo` (no `CAP_NET_RAW` equivalent)
- Use `en0` for Wi-Fi, `lo0` for loopback
- Berkeley Packet Filter (BPF) device permissions may need adjustment for non-root capture

### Linux

- Install `libpcap-dev` (Debian/Ubuntu) or `libpcap-devel` (Fedora/RHEL)
- Use `setcap` to avoid running as root
- `eth0`, `ens33`, etc. for network interfaces; `lo` for loopback
- `AF_PACKET` is used for capture; ensure it's not blocked by security policies

### Windows

- Requires Npcap (not WinPcap) for live capture
- Download from [npcap.com](https://npcap.com)
- Run terminal as Administrator for capture, or install Npcap with "WinPcap API-compatible" mode

## Getting Help

If you're stuck:

1. Enable verbose logging: `RUST_LOG=debug prb ...`
2. Check the [issue tracker](https://github.com/yourusername/prb/issues)
3. Include the full error message, PRB version (`prb --version`), OS, and a minimal reproduction when filing issues
