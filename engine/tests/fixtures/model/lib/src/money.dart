class Money {
  final int cents;
  const Money(this.cents);
}

enum Mood { calm, busy }

/// Shadows `dart:core`'s `Error` for files that import this one.
class Error {}
