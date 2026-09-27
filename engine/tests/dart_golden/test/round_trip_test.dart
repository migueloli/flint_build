import 'dart:convert';

import 'package:flint_dart_golden/annotations_model.dart';
import 'package:flint_dart_golden/enum_values_model.dart';
import 'package:flint_dart_golden/generic_model.dart';
import 'package:flint_dart_golden/options_model.dart';
import 'package:flint_dart_golden/prefixed_model.dart';
import 'package:flint_dart_golden/user_model.dart';
import 'package:test/test.dart';

/// Encodes through `jsonEncode` so the result is exactly what would go over the wire.
Map<String, dynamic> wire(Object? value) =>
    jsonDecode(jsonEncode(value)) as Map<String, dynamic>;

void main() {
  group('User', () {
    final full = <String, dynamic>{
      'id': 1,
      'name': 'Ada',
      'score': 9.5,
      'isActive': true,
      'createdAt': '2026-01-02T03:04:05.000Z',
      'tags': ['a', 'b'],
      'stats': {'posts': 3},
      'addresses': [
        {'city': 'Lisbon', 'zip': '1000'},
      ],
      'home_address': {'city': 'Porto', 'zip': null},
      'work': null,
      'nickname': 'ada',
      'age': 36,
      'rating': 4.0,
      'verified': false,
      'deletedAt': null,
      'luckyNumbers': [7],
      'labels': {'team': 'core'},
      'locale': 'pt',
      'secret': 's3cret',
      'role': 'admin',
      'previousRole': 'guest',
    };

    test('decodes every field', () {
      final user = User.fromJson(full);
      expect(user.id, 1);
      expect(user.score, 9.5);
      expect(user.createdAt, DateTime.utc(2026, 1, 2, 3, 4, 5));
      expect(user.stats, {'posts': 3});
      expect(user.addresses.single.city, 'Lisbon');
      expect(user.home.city, 'Porto');
      expect(user.work, isNull);
      expect(user.rating, 4.0);
      expect(user.role, Role.admin);
      expect(user.previousRole, Role.guest);
    });

    test('round-trips to the same JSON', () {
      expect(wire(User.fromJson(full)), full);
    });

    test('ints are accepted where doubles are declared', () {
      expect(User.fromJson({...full, 'score': 9}).score, 9.0);
    });

    test('applies defaultValue and includeIfNull', () {
      final json =
          User.fromJson({...full, 'locale': null, 'secret': null}).toJson();
      expect(json['locale'], 'unknown');
      expect(json.containsKey('secret'), isFalse);
      expect(json.containsKey('nickname'), isTrue);
    });
  });

  group('Page<T>', () {
    test('uses the generic factories and the converter', () {
      final json = {
        'items': [1, 2],
        'first': 1,
        'fetchedAt': 1767323045000,
      };
      final page = Page<int>.fromJson(json, (value) => value as int);
      expect(page.items, [1, 2]);
      expect(page.fetchedAt,
          DateTime.fromMillisecondsSinceEpoch(1767323045000, isUtc: true));
      expect(wire(page.toJson((value) => value)), json);
    });
  });

  group('class options', () {
    test(
        'explicitToJson, class-level includeIfNull, fromJson/toJson hooks and excluded fields',
        () {
      final json = {
        'origin': {'x': 0, 'y': 0},
        'points': [
          {'x': 1, 'y': 2},
        ],
        'color': '#ff0000',
        'cache': 'ignored',
      };
      final shape = Shape.fromJson(json);
      expect(shape.color, 0xff0000);
      expect(shape.cache, isNull);

      final out = shape.toJson();
      expect(out['origin'], isA<Map<String, dynamic>>());
      expect(out.containsKey('label'), isFalse);
      expect(out.containsKey('cache'), isFalse);
      expect(wire(shape), {
        'origin': {'x': 0, 'y': 0},
        'points': [
          {'x': 1, 'y': 2},
        ],
        'color': '#ff0000',
      });
    });

    test('createFactory: false and createToJson: false', () {
      expect(Event('launch').toJson(), {'name': 'launch'});
      expect(Ping.fromJson({'seq': 3}).seq, 3);
    });
  });

  group('several annotations (R3)', () {
    test('classes and enums with extra annotations are generated', () {
      final json = {
        'frozen': {'id': 7},
        'level': 'lo',
      };
      final tagged = Tagged.fromJson(json);
      expect(tagged.frozen.id, 7);
      expect(tagged.level, Level.low);
      expect(wire(tagged), json);
      expect(Frozen.fromJson({'id': 1}).toJson(), {'id': 1});
    });

    test('only @JsonValue sets an enum constant\'s JSON value', () {
      Map<String, dynamic> json(String level) => {
            'frozen': {'id': 1},
            'level': level,
          };
      expect(Tagged.fromJson(json('mid')).level, Level.medium);
      expect(Tagged.fromJson(json('high')).level, Level.high);
      expect(wire(Tagged.fromJson(json('lo')))['level'], 'lo');
    });
  });

  group('@JsonValue literal types (R6)', () {
    test('int, bool and quoted string values keep their type on the wire', () {
      for (final json in [
        {
          'priority': 2,
          'backup': 1,
          'toggle': true,
          'quote': "it's",
          'votes': {'1': 3},
          'quotes': {"it's": 1}
        },
        {
          'priority': 1,
          'backup': null,
          'toggle': false,
          'quote': 'say "hi"',
          'votes': {},
          'quotes': {}
        },
        {
          'priority': 1,
          'backup': null,
          'toggle': false,
          'quote': 'plain',
          'votes': {'2': 1},
          'quotes': {'plain': 2}
        },
      ]) {
        // Decode like real JSON, so nested maps are Map<String, dynamic>.
        expect(wire(Ticket.fromJson(wire(json))), json);
      }
      final ticket = Ticket.fromJson({
        'priority': 2,
        'toggle': true,
        'quote': "it's",
        'votes': {'1': 3},
        'quotes': <String, dynamic>{}
      });
      expect(ticket.votes, {Priority.low: 3});
      expect(ticket.priority, Priority.high);
      expect(ticket.toggle, Toggle.enabled);
      expect(ticket.quote, Quote.apostrophe);
    });
  });

  group('prefixed types and records (spec 0005 step 2)', () {
    test('m.Money through an import prefix, and a record via @JsonKey hooks', () {
      final json = {
        'total': {'cents': 1500, 'currency': 'EUR'},
        'lines': [
          {'cents': 1000, 'currency': 'EUR'},
          {'cents': 500, 'currency': 'EUR'},
        ],
        'byTax': {
          'vat': {'cents': 300, 'currency': 'EUR'},
        },
        'pages': [1, 3],
      };
      final invoice = Invoice.fromJson(wire(json));
      expect(invoice.total.cents, 1500);
      expect(invoice.pages, (1, 3));
      expect(wire(invoice), json);
    });
  });
}
