import 'package:json_annotation/json_annotation.dart';

// No @JsonEnum and no generated file of its own: cross_file_model.dart gets its own copy of the map.
enum Status {
  @JsonValue('on')
  active,
  inactive,
}
