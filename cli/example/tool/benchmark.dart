import 'dart:io';

void main() async {
  print('🏃 Starting Benchmark Suite...');

  // Ensure we have dependencies
  print('📦 Getting dependencies...');
  await Process.run('fvm', ['dart', 'pub', 'get']);

  // 1. Clean generated files
  await cleanGeneratedFiles();

  // 2. Measure build_runner
  print('\n⏳ Running build_runner (Cold Start)...');
  final buildRunnerTime = await measureCommand('fvm', ['dart', 'run', 'build_runner', 'build', '--delete-conflicting-outputs']);
  print('✅ build_runner finished in ${buildRunnerTime.inMilliseconds}ms');

  // Save output
  await saveOutput('user_model.build_runner.dart');

  // 3. Clean generated files
  await cleanGeneratedFiles();

  // 4. Measure flint_build
  print('\n⏳ Running flint_build (Cold Start)...');
  final flintTime = await measureCommand('fvm', ['dart', 'run', 'flint_build', 'build']);
  print('✅ flint_build finished in ${flintTime.inMilliseconds}ms');

  // Save output
  await saveOutput('user_model.flint.dart');

  // 5. Results
  final multiplier = (buildRunnerTime.inMilliseconds / flintTime.inMilliseconds).toStringAsFixed(1);
  print('\n=======================================');
  print('🏆 RESULTS:');
  print('=======================================');
  print('build_runner: ${buildRunnerTime.inMilliseconds}ms');
  print('flint_build:  ${flintTime.inMilliseconds}ms');
  print('🚀 Flint is ${multiplier}x faster!');
  print('=======================================\n');

  // 6. Diff
  print('🔍 Running Diff (build_runner vs flint_build)...');
  final diffResult = await Process.run('diff', [
    '-u',
    'benchmark_outputs/user_model.build_runner.dart',
    'benchmark_outputs/user_model.flint.dart'
  ]);
  if (diffResult.exitCode == 0) {
    print('✅ Outputs are perfectly identical!');
  } else {
    print('⚠️ Outputs have differences (this is expected due to boilerplate):');
    print(diffResult.stdout);
  }
}

Future<void> cleanGeneratedFiles() async {
  final file = File('lib/user_model.g.dart');
  if (file.existsSync()) {
    file.deleteSync();
  }
}

Future<Duration> measureCommand(String executable, List<String> args) async {
  final stopwatch = Stopwatch()..start();
  final result = await Process.run(executable, args);
  stopwatch.stop();
  if (result.exitCode != 0) {
    print('❌ Command failed: $executable ${args.join(' ')}');
    print(result.stdout);
    print(result.stderr);
    exit(1);
  }
  return stopwatch.elapsed;
}

Future<void> saveOutput(String targetName) async {
  final source = File('lib/user_model.g.dart');
  if (source.existsSync()) {
    final dir = Directory('benchmark_outputs');
    if (!dir.existsSync()) {
      dir.createSync();
    }
    source.copySync('benchmark_outputs/$targetName');
  } else {
    print('⚠️ Expected generated file lib/user_model.g.dart not found!');
  }
}
