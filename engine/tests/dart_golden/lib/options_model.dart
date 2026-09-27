import 'package:json_annotation/json_annotation.dart';

part 'options_model.g.dart';

// Class-level @JsonSerializable options and @JsonKey hooks.

@JsonSerializable()
class Point {
  final int x;
  final int y;

  Point({required this.x, required this.y});

  factory Point.fromJson(Map<String, dynamic> json) => _$PointFromJson(json);
  Map<String, dynamic> toJson() => _$PointToJson(this);
}

@JsonSerializable(explicitToJson: true, includeIfNull: false)
class Shape {
  final Point origin;
  final List<Point> points;
  final String? label;
  @JsonKey(fromJson: _parseColor, toJson: _formatColor)
  final int color;
  @JsonKey(includeFromJson: false, includeToJson: false)
  final String? cache;

  Shape({required this.origin, required this.points, this.label, required this.color, this.cache});

  factory Shape.fromJson(Map<String, dynamic> json) => _$ShapeFromJson(json);
  Map<String, dynamic> toJson() => _$ShapeToJson(this);
}

int _parseColor(Object? value) => int.parse((value as String).substring(1), radix: 16);
String _formatColor(int color) => '#${color.toRadixString(16)}';

@JsonSerializable(createFactory: false)
class Event {
  final String name;

  Event(this.name);

  Map<String, dynamic> toJson() => _$EventToJson(this);
}

@JsonSerializable(createToJson: false)
class Ping {
  final int seq;

  Ping({required this.seq});

  factory Ping.fromJson(Map<String, dynamic> json) => _$PingFromJson(json);
}
