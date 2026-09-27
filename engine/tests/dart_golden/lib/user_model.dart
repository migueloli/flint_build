import 'package:json_annotation/json_annotation.dart';

part 'user_model.g.dart';

// Scalars, collections, nested models, nullability, same-file enums and common @JsonKey options.

@JsonSerializable()
class Address {
  final String city;
  final String? zip;

  Address({required this.city, this.zip});

  factory Address.fromJson(Map<String, dynamic> json) => _$AddressFromJson(json);
  Map<String, dynamic> toJson() => _$AddressToJson(this);
}

@JsonSerializable()
class User {
  final int id;
  final String name;
  final double score;
  final bool isActive;
  final DateTime createdAt;
  final List<String> tags;
  final Map<String, int> stats;
  final List<Address> addresses;
  @JsonKey(name: 'home_address')
  final Address home;
  final Address? work;
  final String? nickname;
  final int? age;
  final double? rating;
  final bool? verified;
  final DateTime? deletedAt;
  final List<int>? luckyNumbers;
  final Map<String, String>? labels;
  @JsonKey(defaultValue: 'unknown')
  final String? locale;
  @JsonKey(includeIfNull: false)
  final String? secret;
  final Role role;
  final Role? previousRole;

  User({
    required this.id,
    required this.name,
    required this.score,
    required this.isActive,
    required this.createdAt,
    required this.tags,
    required this.stats,
    required this.addresses,
    required this.home,
    this.work,
    this.nickname,
    this.age,
    this.rating,
    this.verified,
    this.deletedAt,
    this.luckyNumbers,
    this.labels,
    this.locale,
    this.secret,
    required this.role,
    this.previousRole,
  });

  factory User.fromJson(Map<String, dynamic> json) => _$UserFromJson(json);
  Map<String, dynamic> toJson() => _$UserToJson(this);
}

@JsonEnum()
enum Role {
  @JsonValue('admin')
  admin,
  @JsonValue('member')
  member,
  guest,
}
