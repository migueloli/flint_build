import 'package:json_annotation/json_annotation.dart';

part 'user_model.g.dart';

@JsonSerializable()
class Metadata {
  final String info;
  final int count;

  const Metadata({required this.info, required this.count});

  factory Metadata.fromJson(Map<String, dynamic> json) => _$Metadata(json);
  Map<String, dynamic> toJson() => _$Metadata(this);
}

@JsonSerializable()
class UserModel {
  final int id;
  final String name;
  final double score;
  final bool isActive;
  final DateTime createdAt;
  final List<String> tags;
  final Map<String, int> stats;
  @JsonKey(name: "metadata")
  final Metadata subModel;
  final String? optionalTitle;
  @JsonKey(defaultValue: 'No Description Provided')
  String? mutableDescription;
  int? mutableCount;
  final int? optionalInt;
  final Map<String, int>? optionalStats;
  final List<String>? optionalTags;
  @JsonKey(name: "optionalMetadata")
  final Metadata? optionalSubModel;
  final MyEnum status;
  @JsonKey(includeIfNull: false)
  final String? secretData;

  UserModel({
    required this.id,
    required this.name,
    required this.score,
    required this.isActive,
    required this.createdAt,
    required this.tags,
    required this.stats,
    required this.subModel,
    this.optionalTitle,
    this.mutableDescription,
    this.mutableCount,
    this.optionalInt,
    this.optionalStats,
    this.optionalTags,
    this.optionalSubModel,
    required this.status,
    this.secretData,
  });

  factory UserModel.fromJson(Map<String, dynamic> json) => _$UserModel(json);
  Map<String, dynamic> toJson() => _$UserModel(this);
}

@JsonEnum()
enum MyEnum {
  pending,
  completed,
  cancelled,
  @JsonValue('in_test')
  test,
}
