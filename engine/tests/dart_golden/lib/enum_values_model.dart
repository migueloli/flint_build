import 'package:json_annotation/json_annotation.dart';

part 'enum_values_model.g.dart';

// R6: @JsonValue keeps the literal's type (int, bool, String) and its exact source, so quotes survive.

@JsonEnum()
enum Priority {
  @JsonValue(1)
  low,
  @JsonValue(2)
  high,
}

@JsonEnum()
enum Toggle {
  @JsonValue(true)
  enabled,
  @JsonValue(false)
  disabled,
}

@JsonEnum()
enum Quote {
  @JsonValue("it's")
  apostrophe,
  @JsonValue('say "hi"')
  doubleQuoted,
  plain,
}

@JsonSerializable()
class Ticket {
  final Priority priority;
  final Priority? backup;
  final Toggle toggle;
  final Quote quote;
  // JSON object keys are always strings, whatever the @JsonValue type.
  final Map<Priority, int> votes;
  final Map<Quote, int> quotes;

  Ticket({
    required this.priority,
    this.backup,
    required this.toggle,
    required this.quote,
    this.votes = const {},
    this.quotes = const {},
  });

  factory Ticket.fromJson(Map<String, dynamic> json) => _$TicketFromJson(json);
  Map<String, dynamic> toJson() => _$TicketToJson(this);
}
