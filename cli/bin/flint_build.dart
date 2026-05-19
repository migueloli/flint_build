import 'dart:io';
import 'dart:isolate';

import 'package:path/path.dart' as p;

const String ansiGreen = '\x1B[32m';
const String ansiRed = '\x1B[31m';
const String ansiYellow = '\x1B[33m';
const String ansiCyan = '\x1B[36m';
const String ansiBold = '\x1B[1m';
const String ansiReset = '\x1B[0m';

void main(List<String> args) async {
  final packageUri =
      await Isolate.resolvePackageUri(Uri.parse('package:flint_build/'));
  if (packageUri == null) {
    print(
        '$ansiRed❌ Could not resolve package:flint_build. Make sure it is in your pubspec dependencies.$ansiReset');
    exit(1);
  }

  final cliRoot = p.dirname(packageUri.toFilePath());
  final engineRoot = p.normalize(p.join(cliRoot, '..', 'engine'));

  final binaryName = Platform.isWindows ? 'flint_build.exe' : 'flint_build';
  final releaseBinaryPath = p.join(engineRoot, 'target', 'release', binaryName);
  final debugBinaryPath = p.join(engineRoot, 'target', 'debug', binaryName);

  File binary = File(releaseBinaryPath);

  if (!binary.existsSync()) {
    final debugBinary = File(debugBinaryPath);
    if (debugBinary.existsSync()) {
      print(
          '$ansiYellow⚠️ Release engine not found. Running dev debug engine instead...$ansiReset');
      binary = debugBinary;
    } else {
      print(
          '$ansiCyan⚡ Native Flint engine not found at: ${binary.path}$ansiReset');
      print('$ansiBold📦 Attempting to build Flint from source...$ansiReset');

      try {
        final cargoCheck = await Process.run('cargo', ['--version']);
        if (cargoCheck.exitCode != 0) {
          throw Exception('Cargo not functional');
        }

        print(
            '$ansiYellow🔧 Running "cargo build --release" inside $engineRoot...$ansiReset');

        final buildProcess = await Process.start(
          'cargo',
          ['build', '--release'],
          workingDirectory: engineRoot,
          mode: ProcessStartMode.inheritStdio,
        );

        final buildExitCode = await buildProcess.exitCode;
        if (buildExitCode != 0) {
          print(
              '$ansiRed❌ Automatic compilation failed with exit code $buildExitCode.$ansiReset');
          exit(1);
        }

        binary = File(releaseBinaryPath);
        if (!binary.existsSync()) {
          print(
              '$ansiRed❌ Compilation succeeded but binary was not found at expected location: ${binary.path}$ansiReset');
          exit(1);
        }

        print(
            '$ansiGreen🎉 Successfully compiled and initialized Flint Build Engine!$ansiReset\n');
      } catch (e) {
        print('\n$ansiRed❌ Cargo is not available on your system.$ansiReset');
        print(
            '$ansiBold💡 Please install Rust (https://rustup.rs) or place a pre-compiled native binary inside "engine/target/release/flint_build".$ansiReset');
        exit(1);
      }
    }
  }

  final process = await Process.start(
    binary.path,
    args,
    mode: ProcessStartMode.inheritStdio,
  );

  exit(await process.exitCode);
}
