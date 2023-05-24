module TimeDiff exposing (TimeDiff(..), diff, toString)

import Time


type TimeDiff
    = Days Int (Maybe Int) (Maybe Int) (Maybe Int)
    | Hours Int (Maybe Int) (Maybe Int)
    | Minutes Int (Maybe Int)
    | Seconds Int


justIfNonZeo : Float -> Maybe Int
justIfNonZeo amt =
    if truncate amt > 0 then
        Just (truncate amt)

    else
        Nothing


diff : Time.Posix -> Time.Posix -> TimeDiff
diff now earlier =
    let
        nowMs =
            Time.posixToMillis now

        diffMs =
            nowMs - Time.posixToMillis earlier

        diffDays =
            toFloat diffMs / (1000 * 60 * 60 * 24)

        diffHours =
            (diffDays - toFloat (truncate diffDays)) * 24

        diffMinutes =
            (diffHours - toFloat (truncate diffHours)) * 60

        diffSeconds =
            (diffMinutes - toFloat (truncate diffMinutes)) * 60
    in
    case ( justIfNonZeo diffDays, justIfNonZeo diffHours, justIfNonZeo diffMinutes ) of
        ( Just d, h, m ) ->
            Days d h m (justIfNonZeo diffSeconds)

        ( Nothing, Just h, m ) ->
            Hours h m (justIfNonZeo diffSeconds)

        ( Nothing, Nothing, Just m ) ->
            Minutes m (justIfNonZeo diffSeconds)

        _ ->
            Seconds (truncate diffSeconds)


toString : TimeDiff -> String
toString timeDiff =
    case timeDiff of
        Days d h m s ->
            String.fromInt d ++ " days"

        Hours h m s ->
            String.fromInt h ++ " hours"

        Minutes m s ->
            String.fromInt m ++ " minutes"

        Seconds s ->
            String.fromInt s ++ " seconds"
