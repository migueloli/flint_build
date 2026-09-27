import 'package:json_annotation/json_annotation.dart';
import 'package:meta/meta.dart';

part 'annotations_model.g.dart';

// R3: declarations with more than one annotation, in either order.

class Tag {
  const Tag();
}

class Note {
  final String text;
  const Note(this.text);
}

@immutable
@JsonSerializable()
class Frozen {
  final int id;

  const Frozen({required this.id});

  factory Frozen.fromJson(Map<String, dynamic> json) => _$FrozenFromJson(json);
  Map<String, dynamic> toJson() => _$FrozenToJson(this);
}

@JsonSerializable(explicitToJson: true)
@Tag()
class Tagged {
  final Frozen frozen;
  final Level level;

  Tagged({required this.frozen, required this.level});

  factory Tagged.fromJson(Map<String, dynamic> json) => _$TaggedFromJson(json);
  Map<String, dynamic> toJson() => _$TaggedToJson(this);
}

@Tag()
@JsonEnum()
enum Level {
  @JsonValue('lo')
  @Note('prefer high')
  low,
  @Note('the default')
  @JsonValue('mid')
  medium,
  @Note('no JsonValue, so the name is used')
  high,
}
