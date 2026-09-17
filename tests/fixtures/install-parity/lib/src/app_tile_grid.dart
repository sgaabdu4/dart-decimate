import 'app_tile.dart';

class AppTileGrid extends StatelessWidget {
  const AppTileGrid({required this.tiles});

  final List<AppTile> tiles;

  Widget build(BuildContext context) => Column(children: tiles);
}

