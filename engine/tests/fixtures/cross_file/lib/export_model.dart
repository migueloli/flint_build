import 'package:golden/src/catalog.dart';
import 'package:json_annotation/json_annotation.dart';

part 'export_model.g.dart';

// Spec 0005: an enum and a class reached through a barrel file that `export`s them, imported as
// package:<this package>/…. Neither has an annotation or a generated file of its own.

@JsonSerializable(explicitToJson: true)
class Shelf {
  final Grade level;
  final Label tag;
  final List<Label> tags;
  final Map<Grade, Label> byLevel;

  Shelf({
    required this.level,
    required this.tag,
    required this.tags,
    required this.byLevel,
  });

  factory Shelf.fromJson(Map<String, dynamic> json) => _$ShelfFromJson(json);
  Map<String, dynamic> toJson() => _$ShelfToJson(this);
}
