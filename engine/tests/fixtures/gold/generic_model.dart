import 'package:json_annotation/json_annotation.dart';

part 'generic_model.g.dart';

@JsonSerializable(genericArgumentFactories: true)
class ApiResponse<T> {
  final T data;
  @MyConverter()
  final DateTime date;

  ApiResponse({required this.data, required this.date});
}
