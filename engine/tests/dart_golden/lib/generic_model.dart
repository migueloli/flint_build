import 'package:json_annotation/json_annotation.dart';

part 'generic_model.g.dart';

// Generic classes (fromJsonT / toJsonT parameters) and converter classes from `converters:` in flint.yaml.

class EpochConverter implements JsonConverter<DateTime, int> {
  const EpochConverter();

  @override
  DateTime fromJson(int json) => DateTime.fromMillisecondsSinceEpoch(json, isUtc: true);

  @override
  int toJson(DateTime object) => object.millisecondsSinceEpoch;
}

@JsonSerializable(genericArgumentFactories: true)
class Page<T> {
  final List<T> items;
  final T first;
  @EpochConverter()
  final DateTime fetchedAt;

  Page({required this.items, required this.first, required this.fetchedAt});

  factory Page.fromJson(Map<String, dynamic> json, T Function(Object? json) fromJsonT) =>
      _$PageFromJson(json, fromJsonT);
  Map<String, dynamic> toJson(Object? Function(T value) toJsonT) => _$PageToJson(this, toJsonT);
}
