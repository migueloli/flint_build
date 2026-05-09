// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'user_model.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

Metadata _$MetadataFromJson(Map<String, dynamic> json) => Metadata(
      info: json['info'] as String,
      count: (json['count'] as num).toInt(),
    );

Map<String, dynamic> _$MetadataToJson(Metadata instance) => <String, dynamic>{
      'info': instance.info,
      'count': instance.count,
    };

UserModel _$UserModelFromJson(Map<String, dynamic> json) => UserModel(
      id: (json['id'] as num).toInt(),
      name: json['name'] as String,
      score: (json['score'] as num).toDouble(),
      isActive: json['isActive'] as bool,
      createdAt: DateTime.parse(json['createdAt'] as String),
      tags: (json['tags'] as List<dynamic>).map((e) => e as String).toList(),
      stats: Map<String, int>.from(json['stats'] as Map),
      subModel: Metadata.fromJson(json['metadata'] as Map<String, dynamic>),
      optionalTitle: json['optionalTitle'] as String?,
      mutableDescription:
          json['mutableDescription'] as String? ?? 'No Description Provided',
      mutableCount: (json['mutableCount'] as num?)?.toInt(),
      optionalInt: (json['optionalInt'] as num?)?.toInt(),
      optionalStats: (json['optionalStats'] as Map<String, dynamic>?)?.map(
        (k, e) => MapEntry(k, (e as num).toInt()),
      ),
      optionalTags: (json['optionalTags'] as List<dynamic>?)
          ?.map((e) => e as String)
          .toList(),
      optionalSubModel: json['optionalMetadata'] == null
          ? null
          : Metadata.fromJson(json['optionalMetadata'] as Map<String, dynamic>),
    );

Map<String, dynamic> _$UserModelToJson(UserModel instance) => <String, dynamic>{
      'id': instance.id,
      'name': instance.name,
      'score': instance.score,
      'isActive': instance.isActive,
      'createdAt': instance.createdAt.toIso8601String(),
      'tags': instance.tags,
      'stats': instance.stats,
      'metadata': instance.subModel,
      'optionalTitle': instance.optionalTitle,
      'mutableDescription': instance.mutableDescription,
      'mutableCount': instance.mutableCount,
      'optionalInt': instance.optionalInt,
      'optionalStats': instance.optionalStats,
      'optionalTags': instance.optionalTags,
      'optionalMetadata': instance.optionalSubModel,
    };
