// Every kind of declaration the generator model describes (spec 0007).
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'src/money.dart' as m;
import 'src/money.dart' show Mood, Error;

part 'model_fixture.g.dart';

/// A functional provider, riverpod_generator style.
@riverpod
Future<List<m.Money>> fetchPrices(Ref ref, {required int page, Mood mood = Mood.calm}) async => [];

@Anno(1, 2.5, true, null, 'a', [1, 'x'], {'k': 1}, key: #sym, other: Foo.bar, neg: -1)
@Anno.named(Mood.calm)
int get answer => 42;

set topLevelSetter(int value) {}

const int kMax = 3;
final list = [1, 2];
late String lateVar;
const zero = Duration.zero;

@deprecated
typedef Json = Map<String, dynamic>;
typedef void Callback(int x);
typedef Mapper<T> = T Function(Object?);

/// A union, freezed style.
@freezed
sealed class Shape<T extends Object?> extends Base<T> with Named implements Comparable<Shape<T>> {
  /// A circle.
  const factory Shape.circle({@Default(1) double radius, @JsonKey(name: 'c') required T color, @Default(Mood.calm) Mood mood}) = Circle<T>;
  const factory Shape.square(@Default(2) double side) = _Square;
  const Shape._();

  static const origin = 0;
  final m.Money? price;
  final Error? error;
  late final String label;

  (int, {String name}) get record => (1, name: 'a');
  set label2(String value) {}

  @override
  Future<void> run<R>(int a, [String? b]) async {}
  int operator +(int other) => 0;
  void abstractMethod();
}

mixin Named on Object {
  String get name => 'named';
}

extension ShapeX<T> on Shape<T> {
  bool get isCircle => false;
}

extension type Id(int value) implements Object {}

enum Color with Named {
  /// Red.
  @JsonValue('r')
  red('R'),

  /// Green.
  green('G');

  final String code;
  const Color(this.code);
}
