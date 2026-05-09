import 'dart:io';

import 'package:path/path.dart' as p;

void main(List<String> args) async {
  // 1. Point to the engine binary relative to the CURRENT directory
  // (Assuming you are running the command from inside the 'cli' folder)
  final projectRoot = Directory.current.path;

  // We go up one level from 'cli' then into 'engine'
  final engineDir = p.normalize(
    p.join(projectRoot, '..', 'engine', 'target', 'release'),
  );

  final binaryName = Platform.isWindows ? 'flint_build.exe' : 'flint_build';
  final binaryPath = p.join(engineDir, binaryName);

  final binary = File(binaryPath);

  if (!binary.existsSync()) {
    print('❌ Native engine not found at: $binaryPath');
    print('💡 Currently looking in: ${Directory.current.path}');
    print('💡 Please run "cargo build --release" inside the "engine" folder.');
    exit(1);
  }

  // 2. Forward to the Rust engine
  final process = await Process.start(
    binary.path,
    args,
    mode: ProcessStartMode.inheritStdio,
  );

  exit(await process.exitCode);
}
