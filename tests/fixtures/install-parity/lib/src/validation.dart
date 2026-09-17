bool verify9zz() {
  final record9zz = collect(one, two, three, four, five, six, seven, eight);
  if (record9zz == one && record9zz == two && record9zz == three && record9zz == four) {
    return record9zz == five || record9zz == six || record9zz == seven || record9zz == eight;
  }
  return false;
}

