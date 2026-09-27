import 'package:json_annotation/json_annotation.dart';

import 'src/money.dart' as m;

part 'prefixed_model.g.dart';

// Spec 0005 step 2: prefixed type names keep their dot, and record fields work through @JsonKey hooks.

@JsonSerializable(explicitToJson: true)
class Invoice {
  final m.Money total;
  final List<m.Money> lines;
  final Map<String, m.Money>? byTax;
  @JsonKey(fromJson: _rangeFromJson, toJson: _rangeToJson)
  final (int, int) pages;

  Invoice({required this.total, required this.lines, this.byTax, required this.pages});

  factory Invoice.fromJson(Map<String, dynamic> json) => _$InvoiceFromJson(json);
  Map<String, dynamic> toJson() => _$InvoiceToJson(this);
}

(int, int) _rangeFromJson(Object? json) {
  final list = json as List<dynamic>;
  return (list[0] as int, list[1] as int);
}

List<int> _rangeToJson((int, int) range) => [range.$1, range.$2];
