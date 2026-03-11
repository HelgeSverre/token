{- |
  Haskell Syntax Highlighting Test
  A JSON parser and pretty printer using parser combinators.
-}

module JsonParser
  ( JsonValue(..)
  , parseJson
  , prettyPrint
  , encode
  ) where

import           Data.Char          (digitToInt, isDigit, isHexDigit, isSpace)
import           Data.List          (intercalate)
import           Data.Map.Strict    (Map)
import qualified Data.Map.Strict    as Map
import           Data.Maybe         (fromMaybe)
import           Control.Applicative (Alternative(..))
import           Control.Monad      (void, when)

-- | Represents a JSON value
data JsonValue
  = JsonNull
  | JsonBool Bool
  | JsonNumber Double
  | JsonString String
  | JsonArray [JsonValue]
  | JsonObject (Map String JsonValue)
  deriving (Eq, Show)

-- | Parser type: takes input string, returns parsed value and remaining input
newtype Parser a = Parser { runParser :: String -> Maybe (a, String) }

instance Functor Parser where
  fmap f (Parser p) = Parser $ \input -> do
    (result, rest) <- p input
    Just (f result, rest)

instance Applicative Parser where
  pure x = Parser $ \input -> Just (x, input)
  (Parser pf) <*> (Parser pa) = Parser $ \input -> do
    (f, rest1) <- pf input
    (a, rest2) <- pa rest1
    Just (f a, rest2)

instance Alternative Parser where
  empty = Parser $ const Nothing
  (Parser p1) <|> (Parser p2) = Parser $ \input ->
    p1 input <|> p2 input

instance Monad Parser where
  (Parser pa) >>= f = Parser $ \input -> do
    (a, rest) <- pa input
    runParser (f a) rest

-- Primitive parsers

-- | Parse a single character satisfying a predicate
satisfy :: (Char -> Bool) -> Parser Char
satisfy pred = Parser $ \case
  (c:cs) | pred c -> Just (c, cs)
  _               -> Nothing

-- | Parse a specific character
char :: Char -> Parser Char
char c = satisfy (== c)

-- | Parse a specific string
string :: String -> Parser String
string = traverse char

-- | Parse zero or more whitespace characters
whitespace :: Parser ()
whitespace = void $ many (satisfy isSpace)

-- | Parse a value surrounded by whitespace
lexeme :: Parser a -> Parser a
lexeme p = whitespace *> p <* whitespace

-- | Parse between delimiters
between :: Parser open -> Parser close -> Parser a -> Parser a
between open close p = open *> p <* close

-- | Parse values separated by a delimiter
sepBy :: Parser a -> Parser sep -> Parser [a]
sepBy p sep = sepBy1 p sep <|> pure []

sepBy1 :: Parser a -> Parser sep -> Parser [a]
sepBy1 p sep = (:) <$> p <*> many (sep *> p)

-- JSON parsers

-- | Parse a JSON null value
jsonNull :: Parser JsonValue
jsonNull = JsonNull <$ string "null"

-- | Parse a JSON boolean
jsonBool :: Parser JsonValue
jsonBool = (JsonBool True <$ string "true")
       <|> (JsonBool False <$ string "false")

-- | Parse a JSON number (simplified)
jsonNumber :: Parser JsonValue
jsonNumber = JsonNumber <$> do
  sign   <- fromMaybe "" <$> optional (string "-")
  whole  <- some (satisfy isDigit)
  frac   <- fromMaybe "" <$> optional ((:) <$> char '.' <*> some (satisfy isDigit))
  expo   <- fromMaybe "" <$> optional parseExponent
  let numStr = sign ++ whole ++ frac ++ expo
  case reads numStr of
    [(n, "")] -> pure n
    _         -> empty
  where
    parseExponent = do
      e    <- char 'e' <|> char 'E'
      sign <- fromMaybe '+' <$> optional (char '+' <|> char '-')
      digs <- some (satisfy isDigit)
      pure (e : sign : digs)

-- | Parse a JSON string
jsonString :: Parser JsonValue
jsonString = JsonString <$> quotedString

quotedString :: Parser String
quotedString = between (char '"') (char '"') (many stringChar)
  where
    stringChar = escapedChar <|> satisfy (\c -> c /= '"' && c /= '\\')

    escapedChar :: Parser Char
    escapedChar = char '\\' *> escapeCode

    escapeCode :: Parser Char
    escapeCode = ('"'  <$ char '"')
             <|> ('\\' <$ char '\\')
             <|> ('/'  <$ char '/')
             <|> ('\b' <$ char 'b')
             <|> ('\f' <$ char 'f')
             <|> ('\n' <$ char 'n')
             <|> ('\r' <$ char 'r')
             <|> ('\t' <$ char 't')
             <|> unicodeEscape

    unicodeEscape :: Parser Char
    unicodeEscape = do
      _ <- char 'u'
      digits <- sequence [hexDigit, hexDigit, hexDigit, hexDigit]
      let code = foldl (\acc d -> acc * 16 + digitToInt d) 0 digits
      pure (toEnum code)

    hexDigit = satisfy isHexDigit

-- | Parse a JSON array
jsonArray :: Parser JsonValue
jsonArray = JsonArray <$> between
  (lexeme (char '['))
  (lexeme (char ']'))
  (sepBy jsonValue (lexeme (char ',')))

-- | Parse a JSON object
jsonObject :: Parser JsonValue
jsonObject = JsonObject . Map.fromList <$> between
  (lexeme (char '{'))
  (lexeme (char '}'))
  (sepBy keyValue (lexeme (char ',')))
  where
    keyValue :: Parser (String, JsonValue)
    keyValue = do
      key <- lexeme quotedString
      _   <- lexeme (char ':')
      val <- jsonValue
      pure (key, val)

-- | Parse any JSON value
jsonValue :: Parser JsonValue
jsonValue = lexeme $ jsonNull
                 <|> jsonBool
                 <|> jsonNumber
                 <|> jsonString
                 <|> jsonArray
                 <|> jsonObject

-- | Parse a JSON string, returning Nothing on failure
parseJson :: String -> Maybe JsonValue
parseJson input = case runParser (jsonValue <* whitespace) input of
  Just (value, "") -> Just value
  _                -> Nothing

-- Pretty printer

-- | Pretty print a JSON value with indentation
prettyPrint :: JsonValue -> String
prettyPrint = go 0
  where
    indent n = replicate (n * 2) ' '

    go :: Int -> JsonValue -> String
    go _ JsonNull        = "null"
    go _ (JsonBool True) = "true"
    go _ (JsonBool False)= "false"
    go _ (JsonNumber n)
      | n == fromIntegral (round n) = show (round n :: Integer)
      | otherwise                   = show n
    go _ (JsonString s)  = "\"" ++ escapeString s ++ "\""
    go depth (JsonArray [])  = "[]"
    go depth (JsonArray items) =
      "[\n"
      ++ intercalate ",\n" (map (\item -> indent (depth + 1) ++ go (depth + 1) item) items)
      ++ "\n" ++ indent depth ++ "]"
    go depth (JsonObject obj)
      | Map.null obj = "{}"
      | otherwise =
          "{\n"
          ++ intercalate ",\n" (map (formatEntry (depth + 1)) (Map.toAscList obj))
          ++ "\n" ++ indent depth ++ "}"

    formatEntry :: Int -> (String, JsonValue) -> String
    formatEntry depth (key, val) =
      indent depth ++ "\"" ++ escapeString key ++ "\": " ++ go depth val

-- | Encode a JSON value as a compact string
encode :: JsonValue -> String
encode JsonNull         = "null"
encode (JsonBool True)  = "true"
encode (JsonBool False) = "false"
encode (JsonNumber n)   = show n
encode (JsonString s)   = "\"" ++ escapeString s ++ "\""
encode (JsonArray items) = "[" ++ intercalate "," (map encode items) ++ "]"
encode (JsonObject obj)  = "{" ++ intercalate "," (map encodeEntry (Map.toAscList obj)) ++ "}"
  where encodeEntry (k, v) = "\"" ++ escapeString k ++ "\":" ++ encode v

-- | Escape special characters in a string
escapeString :: String -> String
escapeString = concatMap escapeChar
  where
    escapeChar '"'  = "\\\""
    escapeChar '\\' = "\\\\"
    escapeChar '\n' = "\\n"
    escapeChar '\r' = "\\r"
    escapeChar '\t' = "\\t"
    escapeChar c
      | c < ' '   = "\\u" ++ padHex (showHex' (fromEnum c))
      | otherwise  = [c]

    padHex s = replicate (4 - length s) '0' ++ s
    showHex' n
      | n < 16    = [hexChars !! n]
      | otherwise  = showHex' (n `div` 16) ++ [hexChars !! (n `mod` 16)]
    hexChars = "0123456789abcdef"

-- Type class for JSON serialization
class ToJson a where
  toJson :: a -> JsonValue

instance ToJson Bool where
  toJson = JsonBool

instance ToJson Int where
  toJson = JsonNumber . fromIntegral

instance ToJson Double where
  toJson = JsonNumber

instance ToJson String where
  toJson = JsonString

instance ToJson a => ToJson [a] where
  toJson = JsonArray . map toJson

instance ToJson a => ToJson (Map String a) where
  toJson = JsonObject . Map.map toJson

-- Record with named fields
data Config = Config
  { configHost     :: String
  , configPort     :: Int
  , configDebug    :: Bool
  , configWorkers  :: Int
  , configTimeout  :: Double
  } deriving (Show, Eq)

instance ToJson Config where
  toJson cfg = JsonObject $ Map.fromList
    [ ("host",    toJson $ configHost cfg)
    , ("port",    toJson $ configPort cfg)
    , ("debug",   toJson $ configDebug cfg)
    , ("workers", toJson $ configWorkers cfg)
    , ("timeout", toJson $ configTimeout cfg)
    ]

defaultConfig :: Config
defaultConfig = Config
  { configHost    = "localhost"
  , configPort    = 8080
  , configDebug   = False
  , configWorkers = 4
  , configTimeout = 30.0
  }
