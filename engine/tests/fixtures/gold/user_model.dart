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
  String? mutableDescription;
  int mutableCount;
  final int? optionalInt;
  final Map<String, int>? optionalStats;
  final List<String>? optionalTags;
  @JsonKey(name: "optionalMetadata")
  final Metadata? optionalSubModel;

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
    required this.mutableCount,
    this.optionalInt,
    this.optionalStats,
    this.optionalTags,
    this.optionalSubModel,
  });

  factory UserModel.fromJson(Map<String, dynamic> json) => _$UserModel(json);
  Map<String, dynamic> toJson() => _$UserModel(this);
}
