import 'package:fixture/src/money.dart';

// A file outside lib/: its package imports still resolve.
class Holder {
  final Money money;
  const Holder(this.money);
}
