import 'package:json_annotation/json_annotation.dart';

part 'core_types_model.g.dart';

// Spec 0005 step 3: dart:core types with the conversions json_serializable uses.

@JsonSerializable()
class CoreTypes {
  final num count;
  final num? maybeCount;
  final dynamic anything;
  final Object object;
  final Object? maybeObject;
  final Uri link;
  final Uri? maybeLink;
  final BigInt big;
  final BigInt? maybeBig;
  final Duration wait;
  final Duration? maybeWait;
  final Set<int> ids;
  final Set<String>? maybeTags;
  final Iterable<String> names;
  final Iterable<Uri>? maybeLinks;
  final Map<String, dynamic> extra;
  final List<Duration> timeouts;
  final Map<String, Set<int>> groups;

  CoreTypes({
    required this.count,
    this.maybeCount,
    this.anything,
    required this.object,
    this.maybeObject,
    required this.link,
    this.maybeLink,
    required this.big,
    this.maybeBig,
    required this.wait,
    this.maybeWait,
    required this.ids,
    this.maybeTags,
    required this.names,
    this.maybeLinks,
    required this.extra,
    required this.timeouts,
    required this.groups,
  });

  factory CoreTypes.fromJson(Map<String, dynamic> json) => _$CoreTypesFromJson(json);
  Map<String, dynamic> toJson() => _$CoreTypesToJson(this);
}
