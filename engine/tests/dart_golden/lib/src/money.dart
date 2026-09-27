// A plain class with fromJson/toJson and no annotation, imported with a prefix by prefixed_model.dart.
class Money {
  final int cents;
  final String currency;

  const Money(this.cents, this.currency);

  factory Money.fromJson(Map<String, dynamic> json) =>
      Money(json['cents'] as int, json['currency'] as String);

  Map<String, dynamic> toJson() => {'cents': cents, 'currency': currency};
}
