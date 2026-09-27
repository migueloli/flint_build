import 'package:golden_money/golden_money.dart';
import 'package:golden_money/golden_money.dart' as gm;
import 'package:json_annotation/json_annotation.dart';

part 'external_model.g.dart';

// Spec 0005 step 6: a class from another package, listed in `external_types` in flint.yaml.

@JsonSerializable(explicitToJson: true)
class Wallet {
  final Money balance;
  final Money? limit;
  final List<gm.Money> history;

  Wallet({required this.balance, this.limit, required this.history});

  factory Wallet.fromJson(Map<String, dynamic> json) => _$WalletFromJson(json);
  Map<String, dynamic> toJson() => _$WalletToJson(this);
}
