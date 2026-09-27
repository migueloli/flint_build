import 'package:json_annotation/json_annotation.dart';

import 'src/mood.dart' as m;
import 'src/status.dart';

part 'cross_file_model.g.dart';

// Spec 0005 step 5: enums from other files, through an import prefix, and without @JsonEnum.

enum Size { small, large }

@JsonSerializable()
class Account {
  final Status status;
  final Status? previous;
  final List<Status> history;
  final Map<Status, int> counts;
  final m.Mood mood;
  final Size size;

  Account({
    required this.status,
    this.previous,
    required this.history,
    required this.counts,
    required this.mood,
    required this.size,
  });

  factory Account.fromJson(Map<String, dynamic> json) => _$AccountFromJson(json);
  Map<String, dynamic> toJson() => _$AccountToJson(this);
}
