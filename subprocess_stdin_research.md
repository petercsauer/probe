# Best Practices for Injecting Stdin into Running Subprocesses in Python

## Research Summary (2026-03-12)

This document covers production-ready approaches for writing to stdin of running subprocesses in Python, with focus on asyncio patterns.

---

## 1. Basic Pattern: asyncio.create_subprocess_exec with stdin=PIPE

### Code Pattern

```python
import asyncio

async def run_interactive_process():
    proc = await asyncio.create_subprocess_exec(
        'python', '-u', 'child_script.py',
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )

    # Write to stdin
    proc.stdin.write(b"input data\n")
    await proc.stdin.drain()  # CRITICAL: Must drain after write

    # Read response
    line = await proc.stdout.readline()
    print(f"Response: {line.decode()}")

    # Close stdin when done (signals EOF to child)
    proc.stdin.close()
    await proc.stdin.wait_closed()

    await proc.wait()
```

### Key Points
- **stdin=asyncio.subprocess.PIPE** must be set at creation time
- **stdin.write()** is synchronous but buffers data
- **await stdin.drain()** is CRITICAL - flushes buffer and waits for write to complete
- **stdin.close()** sends EOF to child process
- **await stdin.wait_closed()** ensures close is complete

### Buffering Concerns
1. Data is buffered until `drain()` is called
2. Without `drain()`, writes may not reach the child immediately
3. Child process must flush its stdout (use `-u` flag for Python, or `sys.stdout.flush()`)

---

## 2. Multiple Writes to Same Process

### Code Pattern

```python
async def interactive_session():
    proc = await asyncio.create_subprocess_exec(
        'bash',
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )

    commands = [
        b"echo 'First command'\n",
        b"echo 'Second command'\n",
        b"pwd\n",
        b"exit\n"
    ]

    for cmd in commands:
        proc.stdin.write(cmd)
        await proc.stdin.drain()

        # Read response (may need buffering logic)
        await asyncio.sleep(0.1)  # Allow process to respond

    proc.stdin.close()
    await proc.stdin.wait_closed()

    stdout, stderr = await proc.communicate()
    print(stdout.decode())
```

### Key Points
- Can write multiple times to the same stdin stream
- Must `drain()` after each write for real-time interaction
- Don't call `communicate()` while stdin is still open for writing
- Use `close()` + `wait_closed()` before `communicate()`

### Known Pitfalls
1. **Race Condition**: Writing before child is ready to read
   - Solution: Implement handshake protocol or small delays
2. **Deadlock**: Child blocks on full buffer, parent blocks on reading
   - Solution: Use separate tasks for reading/writing
3. **Premature EOF**: Closing stdin too early
   - Solution: Keep stdin open until all commands sent

---

## 3. Concurrent Read/Write Pattern (Production-Ready)

### Code Pattern

```python
async def bidirectional_communication():
    proc = await asyncio.create_subprocess_exec(
        'python', '-u', 'repl.py',
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )

    async def read_output():
        """Continuous reader - prevents deadlock."""
        try:
            while True:
                line = await proc.stdout.readline()
                if not line:
                    break
                print(f"OUT: {line.decode().strip()}")
        except Exception as e:
            print(f"Read error: {e}")

    async def write_input(commands):
        """Writes commands with proper draining."""
        try:
            for cmd in commands:
                proc.stdin.write(cmd.encode() + b'\n')
                await proc.stdin.drain()
                await asyncio.sleep(0.05)  # Rate limiting
        finally:
            proc.stdin.close()
            await proc.stdin.wait_closed()

    # Run concurrently
    commands = ["command1", "command2", "exit"]
    await asyncio.gather(
        read_output(),
        write_input(commands)
    )

    await proc.wait()
```

### Threading/Async Considerations
- **Use asyncio tasks for concurrent I/O** - prevents blocking
- **Don't mix threading with asyncio subprocess** - stick to asyncio
- **Separate read/write tasks** - critical for interactive processes
- **Use asyncio.gather() or create_task()** for concurrency

### Buffering Issues
1. **Line buffering**: Child must flush or use `-u` flag
2. **Block buffering**: Can cause delays in interactive programs
3. **Solution**: Set `PYTHONUNBUFFERED=1` environment variable

---

## 4. Real-Time Stdin Injection (Long-Running Process)

### Code Pattern

```python
import asyncio
from asyncio import Queue

async def long_running_with_injection():
    """Start process and inject stdin dynamically."""
    proc = await asyncio.create_subprocess_exec(
        'python', '-u', 'long_server.py',
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )

    input_queue = Queue()

    async def stdin_writer():
        """Continuously write from queue to stdin."""
        try:
            while True:
                data = await input_queue.get()
                if data is None:  # Sentinel for shutdown
                    break
                proc.stdin.write(data.encode() + b'\n')
                await proc.stdin.drain()
        finally:
            proc.stdin.close()
            await proc.stdin.wait_closed()

    async def stdout_reader():
        """Continuously read stdout."""
        while True:
            line = await proc.stdout.readline()
            if not line:
                break
            print(f"Server: {line.decode().strip()}")

    # Start I/O tasks
    writer_task = asyncio.create_task(stdin_writer())
    reader_task = asyncio.create_task(stdout_reader())

    # Inject commands dynamically
    await input_queue.put("status")
    await asyncio.sleep(1)
    await input_queue.put("config reload")
    await asyncio.sleep(2)
    await input_queue.put("shutdown")
    await input_queue.put(None)  # Signal shutdown

    await writer_task
    await reader_task
    await proc.wait()
```

### Key Points
- **Queue pattern** allows injecting stdin from anywhere
- **Keep stdin open** by not closing until done
- **Separate writer task** prevents blocking main logic
- **Sentinel value** (None) signals shutdown

### Known Pitfalls
1. **Memory buildup**: Queue grows if writes are too fast
   - Solution: Use bounded Queue with maxsize
2. **Process dies unexpectedly**: stdin.write() raises BrokenPipeError
   - Solution: Catch BrokenPipeError, check proc.returncode
3. **Encoding issues**: Mixing str/bytes
   - Solution: Always encode strings to bytes before write()

---

## 5. Works with asyncio.create_subprocess_exec?

**YES** - All patterns above work with `asyncio.create_subprocess_exec()`.

### Requirements
1. Must specify `stdin=asyncio.subprocess.PIPE` at creation
2. Use `proc.stdin.write()` (not the old subprocess.PIPE pattern)
3. Always `await proc.stdin.drain()` after write
4. Close stdin with `proc.stdin.close()` + `await proc.stdin.wait_closed()`

### Does NOT work
- Setting `stdin=DEVNULL` - no stdin available
- Using `communicate(input=...)` after manual writes - undefined behavior
- Writing after `stdin.close()` - raises ValueError

---

## 6. Common Anti-Patterns to Avoid

### Anti-Pattern 1: Using communicate() for interactive I/O
```python
# WRONG - communicate() is for one-shot input
proc = await asyncio.create_subprocess_exec(...)
stdout, stderr = await proc.communicate(input=b"data\n")
# Cannot write again!
```

**Solution**: Use direct stdin.write() for interactive processes.

### Anti-Pattern 2: Forgetting to drain()
```python
# WRONG - data may not be sent
proc.stdin.write(b"command\n")
# Missing: await proc.stdin.drain()
```

**Solution**: Always drain after write.

### Anti-Pattern 3: Blocking read while writing
```python
# WRONG - can deadlock
proc.stdin.write(b"data\n")
await proc.stdin.drain()
output = await proc.stdout.read()  # Blocks forever if child waits for more input
```

**Solution**: Use concurrent tasks or readline() with timeouts.

### Anti-Pattern 4: Not handling BrokenPipeError
```python
# WRONG - crashes when child exits
while True:
    proc.stdin.write(b"data\n")
    await proc.stdin.drain()
```

**Solution**: Catch BrokenPipeError and check proc.returncode.

---

## 7. Production-Ready Template

```python
import asyncio
from asyncio import Queue, Task
from typing import Optional

class InteractiveSubprocess:
    """Production-ready interactive subprocess with stdin injection."""

    def __init__(self, *args, **kwargs):
        self.args = args
        self.kwargs = kwargs
        self.proc: Optional[asyncio.subprocess.Process] = None
        self.input_queue: Queue = Queue(maxsize=100)
        self.output_queue: Queue = Queue()
        self.writer_task: Optional[Task] = None
        self.reader_task: Optional[Task] = None
        self.running = False

    async def start(self):
        """Start the subprocess and I/O tasks."""
        self.proc = await asyncio.create_subprocess_exec(
            *self.args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            **self.kwargs
        )
        self.running = True
        self.writer_task = asyncio.create_task(self._stdin_writer())
        self.reader_task = asyncio.create_task(self._stdout_reader())

    async def _stdin_writer(self):
        """Write to stdin from queue."""
        try:
            while self.running:
                try:
                    data = await asyncio.wait_for(
                        self.input_queue.get(),
                        timeout=0.1
                    )
                    if data is None:
                        break

                    self.proc.stdin.write(data.encode() + b'\n')
                    await self.proc.stdin.drain()

                except asyncio.TimeoutError:
                    continue
                except BrokenPipeError:
                    print("Child process closed stdin")
                    break
        finally:
            if self.proc and self.proc.stdin:
                self.proc.stdin.close()
                await self.proc.stdin.wait_closed()

    async def _stdout_reader(self):
        """Read stdout continuously."""
        try:
            while True:
                line = await self.proc.stdout.readline()
                if not line:
                    break
                await self.output_queue.put(line.decode().strip())
        except Exception as e:
            print(f"Reader error: {e}")

    async def write(self, data: str):
        """Inject data into stdin."""
        if not self.running:
            raise RuntimeError("Process not running")
        await self.input_queue.put(data)

    async def read(self, timeout: float = 1.0) -> Optional[str]:
        """Read one line from stdout."""
        try:
            return await asyncio.wait_for(
                self.output_queue.get(),
                timeout=timeout
            )
        except asyncio.TimeoutError:
            return None

    async def stop(self):
        """Graceful shutdown."""
        self.running = False
        await self.input_queue.put(None)  # Signal shutdown

        if self.writer_task:
            await self.writer_task
        if self.reader_task:
            self.reader_task.cancel()
            try:
                await self.reader_task
            except asyncio.CancelledError:
                pass

        if self.proc:
            try:
                await asyncio.wait_for(self.proc.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                self.proc.kill()
                await self.proc.wait()

# Usage
async def main():
    proc = InteractiveSubprocess('python', '-u', 'repl.py')
    await proc.start()

    await proc.write("command1")
    response = await proc.read()
    print(f"Response: {response}")

    await proc.write("command2")
    response = await proc.read()
    print(f"Response: {response}")

    await proc.stop()

asyncio.run(main())
```

---

## 8. Summary of Best Practices

### DO
1. Always use `stdin=asyncio.subprocess.PIPE` at creation
2. Call `await stdin.drain()` after every write
3. Use separate async tasks for reading and writing
4. Handle BrokenPipeError when child exits
5. Close stdin explicitly with `close()` + `wait_closed()`
6. Use `-u` flag or `PYTHONUNBUFFERED=1` for Python children
7. Implement timeouts to prevent indefinite blocking

### DON'T
1. Mix `communicate()` with manual stdin writes
2. Write to stdin after closing it
3. Block on reads while writing (use concurrent tasks)
4. Forget to encode strings to bytes
5. Ignore drain() - it's not optional for reliable delivery
6. Use threading with asyncio subprocesses (stick to asyncio)
7. Write without checking if process is still alive

---

## 9. Platform-Specific Notes

### Linux/macOS
- Pipe buffers are typically 64KB
- Writes block when buffer is full
- drain() is essential to prevent blocking

### Windows
- Pipe behavior can differ slightly
- Use `creationflags` for proper process group handling
- May need `bufsize=0` for unbuffered I/O

---

## 10. Debugging Tips

### Enable logging
```python
import logging
logging.basicConfig(level=logging.DEBUG)
asyncio.get_event_loop().set_debug(True)
```

### Check process state
```python
if proc.returncode is not None:
    print(f"Process exited with code {proc.returncode}")
```

### Monitor buffer state
```python
# Check if stdin is writable
if proc.stdin.is_closing():
    print("Stdin is closing or closed")
```

### Test with simple echo server
```python
# echo_server.py
import sys
while True:
    line = sys.stdin.readline()
    if not line:
        break
    print(f"Echo: {line.strip()}", flush=True)
```

---

## References
- Python asyncio subprocess documentation
- PEP 3145 - Asynchronous I/O For subprocess.Popen
- asyncio.StreamWriter.drain() documentation
- Production examples from probe codebase:
  - `/Users/psauer/probe/scripts/orchestrate_v2/runner.py` (lines 375-408)
  - Pattern: create_subprocess_exec with stdout.readline() loop
  - Note: Uses DEVNULL for stdin (non-interactive), but demonstrates proper stdout handling
