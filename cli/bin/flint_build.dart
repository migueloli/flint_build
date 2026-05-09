import 'dart:io';
import 'dart:isolate';

import 'package:path/path.dart' as p;

void main(List<String> args) async {
  final packageUri =
      await Isolate.resolvePackageUri(Uri.parse('package:flint_build/'));
  if (packageUri == null) {
    print(
        '❌ Could not resolve package:flint_build. Make sure it is in your dependencies.');
    exit(1);
  }

  final cliRoot = p.dirname(packageUri.toFilePath());
  final engineDir = p.normalize(
    p.join(cliRoot, '..', 'engine', 'target', 'release'),
  );

  final binaryName = Platform.isWindows ? 'flint_build.exe' : 'flint_build';
  final binaryPath = p.join(engineDir, binaryName);

  final binary = File(binaryPath);

  if (!binary.existsSync()) {
    print('❌ Native engine not found at: $binaryPath');
    print('💡 Please run "cargo build --release" inside the "engine" folder.');
    exit(1);
  }

  final process = await Process.start(
    binary.path,
    args,
    mode: ProcessStartMode.inheritStdio,
  );

  exit(await process.exitCode);
}
