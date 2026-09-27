import 'package:json_annotation/json_annotation.dart';

part 'constructors_model.g.dart';

// Spec 0006: models whose shape depends on the real constructor and on which members are serializable.

// Step 1: every variable of `final int a, b;` is a field; static members and getters aren't.
@JsonSerializable()
class Pair {
  static const zero = 0;
  static int created = 0;
  final int a, b;

  Pair({required this.a, required this.b});

  int get sum => a + b;

  factory Pair.fromJson(Map<String, dynamic> json) => _$PairFromJson(json);
  Map<String, dynamic> toJson() => _$PairToJson(this);
}
