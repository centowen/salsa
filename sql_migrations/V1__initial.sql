CREATE TABLE user (
    "id" INTEGER PRIMARY KEY,
    "username" TEXT NOT NULL,
    "salt" TEXT NOT NULL,
    "hashed_password" TEXT NOT NULL
);
