/// Hand-written JSON methods: a class Flint doesn't generate, used by one it does.
class Label {
  final String name;

  const Label(this.name);

  factory Label.fromJson(Map<String, dynamic> json) => Label(json['name'] as String);

  Map<String, dynamic> toJson() => {'name': name};

  @override
  bool operator ==(Object other) => other is Label && other.name == name;

  @override
  int get hashCode => name.hashCode;
}

class Hidden {}
