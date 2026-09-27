/// A class from "another package", with hand-written `fromJson`/`toJson`.
class Money {
  final int cents;
  final String currency;

  const Money(this.cents, this.currency);

  factory Money.fromJson(Map<String, dynamic> json) =>
      Money(json['cents'] as int, json['currency'] as String);

  Map<String, dynamic> toJson() => {'cents': cents, 'currency': currency};

  @override
  bool operator ==(Object other) =>
      other is Money && other.cents == cents && other.currency == currency;

  @override
  int get hashCode => Object.hash(cents, currency);
}
