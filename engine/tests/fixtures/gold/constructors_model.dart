import 'package:json_annotation/json_annotation.dart';

part 'constructors_model.g.dart';

// Spec 0006: models whose shape depends on the real constructor and on which members are serializable.

// Step 1: every variable of `final int a, b;` is a field; static members and getters aren't.
@JsonSerializable()
class Pair {
  static const zero = 0;
  static int created = 0;
  final int a, b;

  Pair({required this.a, required this.b});

  int get sum => a + b;

  factory Pair.fromJson(Map<String, dynamic> json) => _$PairFromJson(json);
  Map<String, dynamic> toJson() => _$PairToJson(this);
}

// Step 2: fromJson calls the real constructor. Each class matches a row of spec 0006's json_serializable table.

// Positional and optional positional parameters, a constructor default, cascades for writable fields, and
// members fromJson can't set (an initialised final, one set in the initialiser list, a getter) left out.
@JsonSerializable()
class Coords {
  static const origin = 0;
  final int x;
  final int y;
  final int z;
  final List<String> tags = const [];
  final int doubled;
  late String label;
  String? note;
  int count = 0;

  Coords(this.x, this.y, [this.z = 0]) : doubled = x * 2;

  int get sum => x + y;

  factory Coords.fromJson(Map<String, dynamic> json) => _$CoordsFromJson(json);
  Map<String, dynamic> toJson() => _$CoordsToJson(this);
}

@JsonSerializable()
class Opts {
  final int a;
  final int b;
  final String? c;
  final int d;

  Opts(this.a, {required this.b, this.c, this.d = 7});

  factory Opts.fromJson(Map<String, dynamic> json) => _$OptsFromJson(json);
  Map<String, dynamic> toJson() => _$OptsToJson(this);
}

// A private field is skipped; the public getter with the constructor parameter's name is the member.
@JsonSerializable()
class Secret {
  final int visible;
  final int _secret;

  Secret({required this.visible, int secret = 0}) : _secret = secret;

  int get secret => _secret;

  factory Secret.fromJson(Map<String, dynamic> json) => _$SecretFromJson(json);
  Map<String, dynamic> toJson() => _$SecretToJson(this);
}

@JsonSerializable(constructor: 'create')
class Made {
  final int x;

  Made._(this.x);

  factory Made.create({required int x}) => Made._(x);

  factory Made.fromJson(Map<String, dynamic> json) => _$MadeFromJson(json);
  Map<String, dynamic> toJson() => _$MadeToJson(this);
}

// Plain parameters match fields by name.
@JsonSerializable()
class Plain {
  final int x;
  final int y;

  Plain(int x, {int y = 3})
      : x = x,
        y = y;

  factory Plain.fromJson(Map<String, dynamic> json) => _$PlainFromJson(json);
  Map<String, dynamic> toJson() => _$PlainToJson(this);
}

@JsonSerializable()
class PrivKey {
  @JsonKey(includeFromJson: true, includeToJson: true)
  final int _hidden;

  PrivKey(this._hidden);

  int get hidden => _hidden;

  factory PrivKey.fromJson(Map<String, dynamic> json) => _$PrivKeyFromJson(json);
  Map<String, dynamic> toJson() => _$PrivKeyToJson(this);
}

@JsonSerializable()
class LateFinal {
  final int x;
  late final String y;
  @JsonKey(includeToJson: true)
  final int derived;

  LateFinal(this.x) : derived = x + 1;

  factory LateFinal.fromJson(Map<String, dynamic> json) => _$LateFinalFromJson(json);
  Map<String, dynamic> toJson() => _$LateFinalToJson(this);
}

@JsonSerializable()
class Twice {
  final int x;

  Twice(this.x);

  @JsonKey(includeToJson: true)
  int get twice => x * 2;

  factory Twice.fromJson(Map<String, dynamic> json) => _$TwiceFromJson(json);
  Map<String, dynamic> toJson() => _$TwiceToJson(this);
}

// @JsonKey(defaultValue:) wins over the constructor's default.
@JsonSerializable()
class Renamed {
  @JsonKey(name: 'the_x', defaultValue: 5)
  final int x;
  final List<int> items;

  Renamed(this.x, {this.items = const [1]});

  factory Renamed.fromJson(Map<String, dynamic> json) => _$RenamedFromJson(json);
  Map<String, dynamic> toJson() => _$RenamedToJson(this);
}

// Without fromJson, initialised finals and public getters are written; private fields aren't.
@JsonSerializable(createFactory: false)
class ToOnly {
  final int x;
  final List<int> tags = const [];
  final int _offset = 1;

  ToOnly(this.x);

  int get sum => x + _offset;

  get untyped => 'u';

  Map<String, dynamic> toJson() => _$ToOnlyToJson(this);
}

@JsonSerializable()
class PosKey {
  @JsonKey(name: 'X')
  final int x;
  final int? y;

  PosKey(this.x, [this.y]);

  factory PosKey.fromJson(Map<String, dynamic> json) => _$PosKeyFromJson(json);
  Map<String, dynamic> toJson() => _$PosKeyToJson(this);
}

// No constructor declared: Dart's implicit one, and every field set by a cascade. (A `factory fromJson`
// would remove the implicit constructor, so this class uses a static method.)
@JsonSerializable()
class Implicit {
  String? a;
  int b = 0;

  static Implicit fromJson(Map<String, dynamic> json) => _$ImplicitFromJson(json);
  Map<String, dynamic> toJson() => _$ImplicitToJson(this);
}

// The model from spec 0006's Problem section, which generated nothing that compiled.
@JsonSerializable()
class ProblemPoint {
  static const origin = 0;
  final int x;
  final int y;
  final int z;
  final List<String> tags = const [];
  late String label;
  final int _secret;
  final int a, b;

  ProblemPoint(this.x, this.y, [this.z = 0, int secret = 0, this.a = 1, this.b = 2]) : _secret = secret;

  int get sum => x + y + _secret;

  factory ProblemPoint.fromJson(Map<String, dynamic> json) => _$ProblemPointFromJson(json);
  Map<String, dynamic> toJson() => _$ProblemPointToJson(this);
}

// Code review of step 2: a getter/setter pair is writable, and a default that uses a static member is
// qualified, because it's copied into a top-level function.
@JsonSerializable()
class Limits {
  static const defaultLimit = 10;
  final int limit;
  int _level = 0;

  Limits({this.limit = defaultLimit * 2});

  int get level => _level;
  set level(int value) => _level = value;

  factory Limits.fromJson(Map<String, dynamic> json) => _$LimitsFromJson(json);
  Map<String, dynamic> toJson() => _$LimitsToJson(this);
}

// A default made of several syntax nodes (`Speed.fast`, `const Duration(…)`) is copied whole.
@JsonEnum()
enum Speed { slow, fast }

@JsonSerializable()
class Defaults {
  final Speed speed;
  final Duration gap;

  Defaults({this.speed = Speed.fast, this.gap = const Duration(seconds: 2)});

  factory Defaults.fromJson(Map<String, dynamic> json) => _$DefaultsFromJson(json);
  Map<String, dynamic> toJson() => _$DefaultsToJson(this);
}
